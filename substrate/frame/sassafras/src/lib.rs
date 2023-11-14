// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Extension module for Sassafras consensus.
//!
//! Sassafras is a constant-time block production protocol that aims to ensure that
//! there is exactly one block produced with constant time intervals rather multiple
//! or none.
//!
//! We run a lottery to distribute block production slots in an epoch and to fix the
//! order validators produce blocks by the beginning of an epoch.
//!
//! Each validator signs the same VRF input and publish the output onchain. This
//! value is their lottery ticket that can be validated against their public key.
//!
//! We want to keep lottery winners secret, i.e. do not publish their public keys.
//! At the begin of the epoch all the validators tickets are published but not their
//! public keys.
//!
//! A valid tickets are validated when an honest validator reclaims it on block
//! production.
//!
//! To prevent submission of fake tickets, resulting in empty slots, the validator
//! when submitting the ticket accompanies it with a SNARK of the statement: "Here's
//! my VRF output that has been generated using the given VRF input and my secret
//! key. I'm not telling you my keys, but my public key is among those of the
//! nominated validators", that is validated before the lottery.
//!
//! To anonymously publish the ticket to the chain a validator sends their tickets
//! to a random validator who later puts it on-chain as a transaction.

#![deny(warnings)]
#![warn(unused_must_use, unsafe_code, unused_variables, unused_imports, missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

use log::{debug, error, warn};
use scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

use frame_support::{
	dispatch::{DispatchResultWithPostInfo, Pays},
	traits::Get,
	weights::Weight,
	BoundedVec, WeakBoundedVec,
};
use frame_system::{
	offchain::{SendTransactionTypes, SubmitTransaction},
	pallet_prelude::BlockNumberFor,
};
use sp_consensus_sassafras::{
	digests::{ConsensusLog, NextEpochDescriptor, SlotClaim},
	vrf, AuthorityId, Epoch, EpochConfiguration, Randomness, Slot, TicketBody, TicketEnvelope,
	TicketId, RANDOMNESS_LENGTH, SASSAFRAS_ENGINE_ID,
};
use sp_io::hashing;
use sp_runtime::{generic::DigestItem, traits::One, BoundToRuntimeAppPublic};
use sp_std::prelude::Vec;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(all(feature = "std", test))]
mod mock;
#[cfg(all(feature = "std", test))]
mod tests;

pub mod weights;
pub use weights::WeightInfo;

pub use pallet::*;

const LOG_TARGET: &str = "sassafras::runtime 🌳";

// Contextual string used by the VRF to generate per-block randomness.
const RANDOMNESS_VRF_CONTEXT: &[u8] = b"SassafrasRandomness";

// Max length for segments holding unsorted tickets.
const SEGMENT_MAX_SIZE: u32 = 128;

// Convenience type
type AuthoritiesVec<T> = WeakBoundedVec<AuthorityId, <T as Config>::MaxAuthorities>;

/// Tickets metadata.
#[derive(Debug, Default, PartialEq, Encode, Decode, TypeInfo, MaxEncodedLen, Clone, Copy)]
pub struct TicketsMetadata {
	/// Number of outstanding next epoch tickets requiring to be sorted.
	///
	/// These tickets are held by the [`UnsortedSegments`] storage map in segments
	/// containing at most `SEGMENT_MAX_SIZE` items.
	pub unsorted_tickets_count: u32,

	/// Number of tickets available for current and next epoch.
	///
	/// These tickets are held by the [`TicketsIds`] storage map.
	///
	/// The array entry to be used for the current epoch is computed as epoch index modulo 2.
	pub tickets_count: [u32; 2],
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// The Sassafras pallet.
	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configuration parameters.
	#[pallet::config]
	pub trait Config: frame_system::Config + SendTransactionTypes<Call<Self>> {
		/// Amount of slots that each epoch should last.
		#[pallet::constant]
		type EpochLength: Get<u64>;

		/// Max number of authorities allowed.
		#[pallet::constant]
		type MaxAuthorities: Get<u32>;

		/// Epoch change trigger.
		///
		/// Logic to be triggered on every block to query for whether an epoch has ended
		/// and to perform the transition to the next epoch.
		type EpochChangeTrigger: EpochChangeTrigger;

		/// Weight information for all calls of this pallet.
		type WeightInfo: WeightInfo;
	}

	/// Max number of tickets allowed by the configuration.
	///
	/// In practice trims down the `Config::EpochLength` value to at most u32::MAX.
	pub struct MaxTicketsFor<T: Config>(sp_std::marker::PhantomData<T>);

	impl<T: Config> Get<u32> for MaxTicketsFor<T> {
		fn get() -> u32 {
			T::EpochLength::get().try_into().unwrap_or(u32::MAX)
		}
	}

	/// Sassafras runtime errors.
	#[pallet::error]
	pub enum Error<T> {
		/// Submitted configuration is invalid.
		InvalidConfiguration,
	}

	/// Current epoch index.
	#[pallet::storage]
	#[pallet::getter(fn epoch_index)]
	pub type EpochIndex<T> = StorageValue<_, u64, ValueQuery>;

	/// Current epoch authorities.
	#[pallet::storage]
	#[pallet::getter(fn authorities)]
	pub type Authorities<T: Config> = StorageValue<_, AuthoritiesVec<T>, ValueQuery>;

	/// Next epoch authorities.
	#[pallet::storage]
	#[pallet::getter(fn next_authorities)]
	pub type NextAuthorities<T: Config> = StorageValue<_, AuthoritiesVec<T>, ValueQuery>;

	/// First block slot number.
	///
	/// As the slots may not be zero-based, we record the slot value for the fist block.
	/// This allows to always compute relative indices for epochs and slots.
	#[pallet::storage]
	#[pallet::getter(fn genesis_slot)]
	pub type GenesisSlot<T> = StorageValue<_, Slot, ValueQuery>;

	/// Current block slot number.
	#[pallet::storage]
	#[pallet::getter(fn current_slot)]
	pub type CurrentSlot<T> = StorageValue<_, Slot, ValueQuery>;

	/// Current epoch randomness.
	#[pallet::storage]
	#[pallet::getter(fn randomness)]
	pub type CurrentRandomness<T> = StorageValue<_, Randomness, ValueQuery>;

	/// Next epoch randomness.
	#[pallet::storage]
	#[pallet::getter(fn next_randomness)]
	pub type NextRandomness<T> = StorageValue<_, Randomness, ValueQuery>;

	/// Randomness accumulator.
	///
	/// During block execution doesn't include randomness which ships within that block header.
	#[pallet::storage]
	pub type RandomnessAccumulator<T> = StorageValue<_, Randomness, ValueQuery>;

	/// The configuration for the current epoch.
	#[pallet::storage]
	#[pallet::getter(fn config)]
	pub type EpochConfig<T> = StorageValue<_, EpochConfiguration, ValueQuery>;

	/// The configuration for the next epoch.
	#[pallet::storage]
	#[pallet::getter(fn next_config)]
	pub type NextEpochConfig<T> = StorageValue<_, EpochConfiguration>;

	/// Pending epoch configuration change that will be set as `NextEpochConfig` when the next
	/// epoch is enacted.
	///
	/// In other words, a config change submitted during epoch N will be enacted on epoch N+2.
	/// This is to maintain coherence for already submitted tickets for epoch N+1 that where
	/// computed using configuration parameters stored for epoch N+1.
	#[pallet::storage]
	pub type PendingEpochConfigChange<T> = StorageValue<_, EpochConfiguration>;

	/// Stored tickets metadata.
	#[pallet::storage]
	pub type TicketsMeta<T> = StorageValue<_, TicketsMetadata, ValueQuery>;

	/// Tickets identifiers map.
	///
	/// The map holds tickets ids for the current and next epoch.
	///
	/// The key is a tuple composed by:
	/// - `u8` equal to epoch's index modulo 2;
	/// - `u32` equal to the ticket's index in a lexicographically sorted list of epoch's tickets.
	///
	/// Epoch X first N-th ticket has key (X mod 2, N)
	///
	/// Note that the ticket's index doesn't directly correspond to the slot index within the epoch.
	/// The assigment is computed dynamically using an *outside-in* strategy.
	///
	/// Be aware that entries within this map are never removed, only overwritten.
	/// Last element index should be fetched from the [`TicketsMeta`] value.
	#[pallet::storage]
	pub type TicketsIds<T> = StorageMap<_, Identity, (u8, u32), TicketId>;

	/// Tickets to be used for current and next epoch.
	#[pallet::storage]
	pub type TicketsData<T> = StorageMap<_, Identity, TicketId, TicketBody>;

	/// Next epoch tickets unsorted segments.
	///
	/// Contains lists of tickets where each list represents a batch of tickets
	/// received via the `submit_tickets` extrinsic.
	///
	/// Each segment has max length [`SEGMENT_MAX_SIZE`].
	#[pallet::storage]
	pub type UnsortedSegments<T: Config> =
		StorageMap<_, Identity, u32, BoundedVec<TicketId, ConstU32<SEGMENT_MAX_SIZE>>, ValueQuery>;

	/// The most recently set of tickets which are candidates to become the next
	/// epoch tickets.
	#[pallet::storage]
	pub type SortedCandidates<T> =
		StorageValue<_, BoundedVec<TicketId, MaxTicketsFor<T>>, ValueQuery>;

	/// Parameters used to construct the epoch's ring verifier.
	///
	/// In practice: Updatable Universal Reference String and the seed.
	#[pallet::storage]
	#[pallet::getter(fn ring_context)]
	pub type RingContext<T: Config> = StorageValue<_, vrf::RingContext>;

	/// Ring verifier data for the current epoch.
	#[pallet::storage]
	pub type RingVerifierData<T: Config> = StorageValue<_, vrf::RingVerifierData>;

	/// Slot claim vrf-preoutput used to generate per-slot randomness.
	///
	/// The value is ephemeral and is cleared on block finalization.
	#[pallet::storage]
	pub(crate) type ClaimTemporaryData<T> = StorageValue<_, vrf::VrfOutput>;

	/// Genesis configuration for Sassafras protocol.
	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		/// Genesis authorities.
		pub authorities: Vec<AuthorityId>,
		/// Genesis epoch configuration.
		pub epoch_config: EpochConfiguration,
		/// Phantom config
		#[serde(skip)]
		pub _phantom: sp_std::marker::PhantomData<T>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			EpochConfig::<T>::put(self.epoch_config);
			Pallet::<T>::initialize_genesis_authorities(&self.authorities);

			#[cfg(feature = "construct-dummy-ring-context")]
			{
				debug!(target: LOG_TARGET, "Constructing dummy ring context");
				let ring_ctx = vrf::RingContext::new_testing();
				RingContext::<T>::put(ring_ctx);
				Pallet::<T>::update_ring_verifier(&self.authorities);
			}
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(block_num: BlockNumberFor<T>) -> Weight {
			let claim = <frame_system::Pallet<T>>::digest()
				.logs
				.iter()
				.filter_map(|item| item.pre_runtime_try_to::<SlotClaim>(&SASSAFRAS_ENGINE_ID))
				.next()
				.expect("Valid block must have a slot claim. qed");

			CurrentSlot::<T>::put(claim.slot);

			// As the slots may not be zero-based, we need to keep track of what is the
			// slot used for the first block.
			if frame_system::Pallet::<T>::block_number() == One::one() {
				GenesisSlot::<T>::put(claim.slot);

				// Deposit a log as this is the first block in first epoch.
				let next_epoch = NextEpochDescriptor {
					randomness: Self::next_randomness(),
					authorities: Self::next_authorities().into_inner(),
					config: None,
				};
				Self::deposit_next_epoch_descriptor_digest(next_epoch);
			}

			let randomness_output = claim
				.vrf_signature
				.outputs
				.get(0)
				.expect("Valid claim must have vrf signature; qed");
			ClaimTemporaryData::<T>::put(randomness_output);

			T::WeightInfo::on_initialize() +
				T::EpochChangeTrigger::trigger::<T>(block_num).unwrap_or_default()
		}

		fn on_finalize(_: BlockNumberFor<T>) {
			// At the end of the block, we can safely include the current slot randomness
			// to the randomness accumulator. If we've determined that this block was the
			// first in a new epoch, the changeover logic has already occurred at this point
			// (i.e. `enact_epoch_change` has already been called).
			let randomness_input = vrf::slot_claim_input(
				&Self::randomness(),
				CurrentSlot::<T>::get(),
				EpochIndex::<T>::get(),
			);
			let randomness_output = ClaimTemporaryData::<T>::take()
				.expect("Finalization is called after initialization; qed");
			let randomness = randomness_output
				.make_bytes::<RANDOMNESS_LENGTH>(RANDOMNESS_VRF_CONTEXT, &randomness_input);
			Self::deposit_slot_randomness(&randomness);

			// Check if we are in the epoch's second half.
			// If so, start sorting the next epoch tickets.
			let epoch_length = T::EpochLength::get();
			let current_slot_idx = Self::current_slot_index();
			if current_slot_idx >= epoch_length / 2 {
				let mut metadata = TicketsMeta::<T>::get();
				if metadata.unsorted_tickets_count != 0 {
					let epoch_idx = EpochIndex::<T>::get() + 1;
					let epoch_tag = (epoch_idx & 1) as u8;
					let slots_left = epoch_length.checked_sub(current_slot_idx).unwrap_or(1);
					Self::sort_tickets(
						metadata
							.unsorted_tickets_count
							.div_ceil(SEGMENT_MAX_SIZE * slots_left as u32),
						epoch_tag,
						&mut metadata,
					);
					TicketsMeta::<T>::set(metadata);
				}
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Submit next epoch tickets candidates.
		///
		/// The number of tickets allowed to be submitted in one call is equal to the epoch length.
		// TODO: maybe we must be more restrictive?
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::submit_tickets(tickets.len() as u32))]
		pub fn submit_tickets(
			origin: OriginFor<T>,
			tickets: BoundedVec<TicketEnvelope, MaxTicketsFor<T>>,
		) -> DispatchResultWithPostInfo {
			ensure_none(origin)?;

			debug!(target: LOG_TARGET, "Received {} tickets", tickets.len());

			let Some(verifier) = RingVerifierData::<T>::get().map(|v| v.into()) else {
				warn!(target: LOG_TARGET, "Ring verifier key not initialized");
				return Err("Ring verifier key not initialized".into())
			};

			let next_authorities = Self::next_authorities();

			// Check tickets score
			let next_config = Self::next_config().unwrap_or_else(|| Self::config());
			// Current slot should be less than half of epoch length.
			let epoch_length = T::EpochLength::get();
			let ticket_threshold = sp_consensus_sassafras::ticket_id_threshold(
				next_config.redundancy_factor,
				epoch_length as u32,
				next_config.attempts_number,
				next_authorities.len() as u32,
			);

			// Get next epoch params
			let randomness = NextRandomness::<T>::get();
			let epoch_idx = EpochIndex::<T>::get() + 1;

			let mut valid_tickets = BoundedVec::with_max_capacity();
			for ticket in tickets {
				debug!(target: LOG_TARGET, "Checking ring proof");

				let ticket_id_input =
					vrf::ticket_id_input(&randomness, ticket.body.attempt_idx, epoch_idx);
				let Some(ticket_id_output) = ticket.signature.outputs.get(0) else {
					debug!(target: LOG_TARGET, "Missing ticket vrf output from ring signature");
					continue
				};
				let ticket_id = vrf::make_ticket_id(&ticket_id_input, &ticket_id_output);
				if ticket_id >= ticket_threshold {
					debug!(target: LOG_TARGET, "Ignoring ticket over threshold ({:032x} >= {:032x})", ticket_id, ticket_threshold);
					continue
				}

				if TicketsData::<T>::contains_key(ticket_id) {
					debug!(target: LOG_TARGET, "Ignoring duplicate ticket ({:032x})", ticket_id);
					continue
				}

				let sign_data = vrf::ticket_body_sign_data(&ticket.body, ticket_id_input);

				if ticket.signature.ring_vrf_verify(&sign_data, &verifier) {
					TicketsData::<T>::set(ticket_id, Some(ticket.body));
					valid_tickets
						.try_push(ticket_id)
						.expect("input segment has same length as bounded destination vector; qed");
				} else {
					debug!(target: LOG_TARGET, "Proof verification failure");
				}
			}

			if !valid_tickets.is_empty() {
				Self::append_tickets(valid_tickets);
			}

			Ok(Pays::No.into())
		}

		/// Plan an epoch config change.
		///
		/// The epoch config change is recorded and will be announced at the begin of the
		/// next epoch together with next epoch authorities information.
		/// In other words the configuration will be activated one epoch after.
		/// Multiple calls to this method will replace any existing planned config change that had
		/// not been enacted yet.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::plan_config_change())]
		pub fn plan_config_change(
			origin: OriginFor<T>,
			config: EpochConfiguration,
		) -> DispatchResult {
			ensure_root(origin)?;

			ensure!(
				config.redundancy_factor != 0 && config.attempts_number != 0,
				Error::<T>::InvalidConfiguration
			);
			PendingEpochConfigChange::<T>::put(config);
			Ok(())
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			let Call::submit_tickets { tickets } = call else {
				return InvalidTransaction::Call.into()
			};

			// Discard tickets not coming from the local node or that are not
			// yet included in a block
			debug!(
				target: LOG_TARGET,
				"Validating unsigned from {} source",
				match source {
					TransactionSource::Local => "local",
					TransactionSource::InBlock => "in-block",
					TransactionSource::External => "external",
				}
			);

			if source == TransactionSource::External {
				// TODO @davxy: BRAINSTORM this `Local` requirement...
				// If we only allow these txs on block production, then there is less chance to
				// submit our tickets if we don't have enough authoring slots.
				// If we have 0 slots => we have zero chances.
				// Maybe this is one valid reason to introduce proxies.
				// In short the question is >>> WHO HAS THE RIGHT TO SUBMIT A TICKET? <<<
				//  A) The current epoch validators
				//  B) Doesn't matter as far as the tickets are good (i.e. RVRF verify is ok)
				// Maybe we also provide a signed extrinsic to submit tickets
				// where the submitter doesn't pay if the tickets are good?
				warn!(
					target: LOG_TARGET,
					"Rejecting unsigned `submit_tickets` transaction from an external source",
				);
				return InvalidTransaction::BadSigner.into()
			}

			// Current slot should be less than half of epoch length.
			let epoch_length = T::EpochLength::get();

			let current_slot_idx = Self::current_slot_index();
			if current_slot_idx > epoch_length / 2 {
				warn!(target: LOG_TARGET, "Timeout to propose tickets, bailing out.",);
				return InvalidTransaction::Stale.into()
			}

			// This should be set such that it is discarded after the first epoch half
			let tickets_longevity = epoch_length / 2 - current_slot_idx;
			let tickets_tag = tickets.using_encoded(|bytes| hashing::blake2_256(bytes));

			ValidTransaction::with_tag_prefix("Sassafras")
				.priority(TransactionPriority::max_value())
				.longevity(tickets_longevity)
				.and_provides(tickets_tag)
				.propagate(true)
				.build()
		}
	}
}

// Inherent methods
impl<T: Config> Pallet<T> {
	/// Determine whether an epoch change should take place at this block.
	///
	/// Assumes that initialization has already taken place.
	pub fn should_end_epoch(block_num: BlockNumberFor<T>) -> bool {
		// The epoch has technically ended during the passage of time between this block and the
		// last, but we have to "end" the epoch now, since there is no earlier possible block we
		// could have done it.
		//
		// The exception is for block 1: the genesis has slot 0, so we treat epoch 0 as having
		// started at the slot of block 1. We want to use the same randomness and validator set as
		// signalled in the genesis, so we don't rotate the epoch.
		block_num > One::one() && Self::current_slot_index() >= T::EpochLength::get()
	}

	/// Current slot index relative to the current epoch.
	fn current_slot_index() -> u64 {
		Self::slot_index(CurrentSlot::<T>::get())
	}

	/// Slot index with respect to current epoch.
	fn slot_index(slot: Slot) -> u64 {
		slot.checked_sub(Self::current_epoch_start().into()).unwrap_or(u64::MAX)
	}

	/// Finds the start slot of the current epoch.
	///
	/// Only guaranteed to give correct results after `initialize` of the first
	/// block in the chain (as its result is based off of `GenesisSlot`).
	fn current_epoch_start() -> Slot {
		Self::epoch_start(EpochIndex::<T>::get())
	}

	/// Get the epoch's first slot.
	fn epoch_start(epoch_index: u64) -> Slot {
		const PROOF: &str = "slot number is u64; it should relate in some way to wall clock time; \
							 if u64 is not enough we should crash for safety; qed.";

		let epoch_start = epoch_index.checked_mul(T::EpochLength::get()).expect(PROOF);
		epoch_start.checked_add(*GenesisSlot::<T>::get()).expect(PROOF).into()
	}

	pub(crate) fn update_ring_verifier(authorities: &[AuthorityId]) {
		debug!(target: LOG_TARGET, "Loading ring context");
		let Some(ring_ctx) = RingContext::<T>::get() else {
			debug!(target: LOG_TARGET, "Ring context not initialized");
			return
		};

		let pks: Vec<_> = authorities.iter().map(|auth| *auth.as_ref()).collect();

		debug!(target: LOG_TARGET, "Building ring verifier (ring size: {})", pks.len());
		let verifier_data = ring_ctx
			.verifier_data(&pks)
			.expect("Failed to build ring verifier. This is a bug");

		RingVerifierData::<T>::put(verifier_data);
	}

	/// Enact an epoch change.
	///
	/// Should be done on every block where `should_end_epoch` has returned `true`, and the caller
	/// is the only caller of this function.
	///
	/// Typically, this is not handled directly, but by a higher-level component implementing the
	/// `EpochChangeTrigger` or `OneSessionHandler` trait.
	///
	/// If we detect one or more skipped epochs the policy is to use the authorities and values
	/// from the first skipped epoch. The tickets are invalidated.
	pub(crate) fn enact_epoch_change(
		authorities: WeakBoundedVec<AuthorityId, T::MaxAuthorities>,
		next_authorities: WeakBoundedVec<AuthorityId, T::MaxAuthorities>,
	) {
		if next_authorities != authorities {
			Self::update_ring_verifier(&next_authorities);
		}

		// Update authorities
		Authorities::<T>::put(&authorities);
		NextAuthorities::<T>::put(&next_authorities);

		// Update epoch index
		let mut epoch_idx = EpochIndex::<T>::get()
			.checked_add(1)
			.expect("epoch indices will never reach 2^64 before the death of the universe; qed");

		let slot_idx = CurrentSlot::<T>::get().saturating_sub(Self::epoch_start(epoch_idx));
		if slot_idx >= T::EpochLength::get() {
			// Detected one or more skipped epochs, clear tickets data and recompute epoch index.
			Self::reset_tickets_data();
			let skipped_epochs = u64::from(slot_idx) / T::EpochLength::get();
			epoch_idx += skipped_epochs;
			warn!(target: LOG_TARGET, "Detected {} skipped epochs, resuming from epoch {}", skipped_epochs, epoch_idx);
		}

		let mut tickets_metadata = TicketsMeta::<T>::get();

		EpochIndex::<T>::put(epoch_idx);

		let next_epoch_index = epoch_idx
			.checked_add(1)
			.expect("epoch indices will never reach 2^64 before the death of the universe; qed");

		// Updates current epoch randomness and computes the *next* epoch randomness.
		let next_randomness = Self::update_epoch_randomness(next_epoch_index);

		if let Some(config) = NextEpochConfig::<T>::take() {
			EpochConfig::<T>::put(config);
		}

		let next_config = PendingEpochConfigChange::<T>::take();
		if let Some(next_config) = next_config {
			NextEpochConfig::<T>::put(next_config);
		}

		// After we update the current epoch, we signal the *next* epoch change
		// so that nodes can track changes.
		let next_epoch = NextEpochDescriptor {
			randomness: next_randomness,
			authorities: next_authorities.into_inner(),
			config: next_config,
		};
		Self::deposit_next_epoch_descriptor_digest(next_epoch);

		let epoch_tag = (epoch_idx & 1) as u8;
		// Optionally finish sorting
		if tickets_metadata.unsorted_tickets_count != 0 {
			Self::sort_tickets(u32::MAX, epoch_tag, &mut tickets_metadata);
		}

		// Clear the "prev ≡ next (mod 2)" epoch tickets counter and bodies.
		// Ids are left since are just cyclically overwritten on-the-go.
		let next_epoch_tag = epoch_tag ^ 1;
		let prev_epoch_tickets_count = &mut tickets_metadata.tickets_count[next_epoch_tag as usize];
		if *prev_epoch_tickets_count != 0 {
			for idx in 0..*prev_epoch_tickets_count {
				if let Some(id) = TicketsIds::<T>::get((next_epoch_tag, idx)) {
					TicketsData::<T>::remove(id);
				}
			}
			*prev_epoch_tickets_count = 0;
			TicketsMeta::<T>::set(tickets_metadata);
		}
	}

	// Call this function on epoch change to enact current epoch randomness.
	//
	// Returns the next epoch randomness.
	fn update_epoch_randomness(next_epoch_index: u64) -> Randomness {
		let curr_epoch_randomness = NextRandomness::<T>::get();
		CurrentRandomness::<T>::put(curr_epoch_randomness);

		let accumulator = RandomnessAccumulator::<T>::get();

		let mut buf = [0; 2 * RANDOMNESS_LENGTH + 8];
		buf[..RANDOMNESS_LENGTH].copy_from_slice(&accumulator[..]);
		buf[RANDOMNESS_LENGTH..2 * RANDOMNESS_LENGTH].copy_from_slice(&curr_epoch_randomness[..]);
		buf[2 * RANDOMNESS_LENGTH..].copy_from_slice(&next_epoch_index.to_le_bytes());

		let next_randomness = hashing::blake2_256(&buf);
		NextRandomness::<T>::put(&next_randomness);

		next_randomness
	}

	// Deposit per-slot randomness.
	fn deposit_slot_randomness(randomness: &Randomness) {
		let accumulator = RandomnessAccumulator::<T>::get();

		let mut buf = [0; 2 * RANDOMNESS_LENGTH];
		buf[..RANDOMNESS_LENGTH].copy_from_slice(&accumulator[..]);
		buf[RANDOMNESS_LENGTH..].copy_from_slice(&randomness[..]);

		let accumulator = hashing::blake2_256(&buf);
		RandomnessAccumulator::<T>::put(accumulator);
	}

	// Deposit next epoch descriptor in the block header digest.
	fn deposit_next_epoch_descriptor_digest(desc: NextEpochDescriptor) {
		let item = ConsensusLog::NextEpochData(desc);
		let log = DigestItem::Consensus(SASSAFRAS_ENGINE_ID, item.encode());
		<frame_system::Pallet<T>>::deposit_log(log)
	}

	// Initialize authorities on genesis phase.
	fn initialize_genesis_authorities(authorities: &[AuthorityId]) {
		// Genesis authorities may have been initialized via other means (e.g. via session pallet).
		// If this function has already been called with some authorities, then the new list
		// should be match the previously set one.
		let prev_authorities = Authorities::<T>::get();
		if !prev_authorities.is_empty() {
			if prev_authorities.as_slice() == authorities {
				return
			} else {
				panic!("Authorities were already initialized");
			}
		}

		let authorities = WeakBoundedVec::try_from(authorities.to_vec())
			.expect("Initial number of authorities should be lower than T::MaxAuthorities");
		Authorities::<T>::put(&authorities);
		NextAuthorities::<T>::put(&authorities);
	}

	/// Current epoch information.
	pub fn current_epoch() -> Epoch {
		let index = EpochIndex::<T>::get();
		Epoch {
			index,
			start: Self::epoch_start(index),
			length: T::EpochLength::get(),
			authorities: Self::authorities().into_inner(),
			randomness: Self::randomness(),
			config: Self::config(),
		}
	}

	/// Next epoch information.
	pub fn next_epoch() -> Epoch {
		let index = EpochIndex::<T>::get()
			.checked_add(1)
			.expect("epoch indices will never reach 2^64 before the death of the universe; qed");
		Epoch {
			index,
			start: Self::epoch_start(index),
			length: T::EpochLength::get(),
			authorities: Self::next_authorities().into_inner(),
			randomness: Self::next_randomness(),
			config: Self::next_config().unwrap_or_else(|| Self::config()),
		}
	}

	/// Fetch expected ticket-id for the given slot according to an "outside-in" sorting strategy.
	///
	/// Given an ordered sequence of tickets [t0, t1, t2, ..., tk] to be assigned to n slots,
	/// with n >= k, then the tickets are assigned to the slots according to the following
	/// strategy:
	///
	/// slot-index  : [ 0,  1,  2, ............ , n ]
	/// tickets     : [ t1, t3, t5, ... , t4, t2, t0 ].
	///
	/// With slot-index computed as `epoch_start() - slot`.
	///
	/// If `slot` value falls within the current epoch then we fetch tickets from the current epoch
	/// tickets list.
	///
	/// If `slot` value falls within the next epoch then we fetch tickets from the next epoch
	/// tickets ids list. Note that in this case we may have not finished receiving all the tickets
	/// for that epoch yet. The next epoch tickets should be considered "stable" only after the
	/// current epoch first half slots were elapsed (see `submit_tickets_unsigned_extrinsic`).
	///
	/// Returns `None` if, according to the sorting strategy, there is no ticket associated to the
	/// specified slot-index (happend if a ticket falls in the middle of an epoch and n > k),
	/// or if the slot falls beyond the next epoch.
	///
	/// Before importing the first block this returns `None`.
	pub fn slot_ticket_id(slot: Slot) -> Option<TicketId> {
		if frame_system::Pallet::<T>::block_number() < One::one() {
			return None
		}
		let epoch_idx = EpochIndex::<T>::get();
		let epoch_len = T::EpochLength::get();
		let mut slot_idx = Self::slot_index(slot);
		let mut tickets_meta = TicketsMeta::<T>::get();

		let get_ticket_idx = |slot_idx| {
			let ticket_idx = if slot_idx < epoch_len / 2 {
				2 * slot_idx + 1
			} else {
				2 * (epoch_len - (slot_idx + 1))
			};
			debug!(
				target: LOG_TARGET,
				"slot-idx {} <-> ticket-idx {}",
				slot_idx,
				ticket_idx
			);
			ticket_idx as u32
		};

		let mut epoch_tag = (epoch_idx & 1) as u8;

		if epoch_len <= slot_idx && slot_idx < 2 * epoch_len {
			// Try to get a ticket for the next epoch. Since its state values were not enacted yet,
			// we may have to finish sorting the tickets.
			epoch_tag ^= 1;
			slot_idx -= epoch_len;
			if tickets_meta.unsorted_tickets_count != 0 {
				Self::sort_tickets(u32::MAX, epoch_tag, &mut tickets_meta);
				TicketsMeta::<T>::set(tickets_meta);
			}
		} else if slot_idx >= 2 * epoch_len {
			return None
		}

		let ticket_idx = get_ticket_idx(slot_idx);
		if ticket_idx < tickets_meta.tickets_count[epoch_tag as usize] {
			TicketsIds::<T>::get((epoch_tag, ticket_idx))
		} else {
			None
		}
	}

	/// Returns ticket id and data associated to the given `slot`.
	///
	/// Refer to the `slot_ticket_id` documentation for the slot-ticket association
	/// criteria.
	pub fn slot_ticket(slot: Slot) -> Option<(TicketId, TicketBody)> {
		Self::slot_ticket_id(slot).and_then(|id| TicketsData::<T>::get(id).map(|body| (id, body)))
	}

	// Lexicographically sort the tickets which belong to the next epoch.
	//
	// Tickets are fetched from at most `max_segments` segments.
	//
	// The resulting sorted vector is optionally truncated to contain at most `MaxTickets`
	// entries. If all the unsorted segments are consumed then the sorted vector is
	// saved as the next epoch tickets, else it is saved to be used by next calls to
	// this function.
	pub(crate) fn sort_tickets(max_segments: u32, epoch_tag: u8, metadata: &mut TicketsMetadata) {
		let mut unsorted_segments_count =
			metadata.unsorted_tickets_count.div_ceil(SEGMENT_MAX_SIZE);
		let max_segments = max_segments.min(unsorted_segments_count);
		let max_tickets = MaxTicketsFor::<T>::get() as usize;

		// Fetch the sorted candidates (if any).
		let mut sorted_candidates = SortedCandidates::<T>::take().into_inner();

		let mut require_sort = max_segments != 0;

		// There is an upper bound to check only if we already sorted the max number
		// of allowed tickets.
		let mut upper_bound = *sorted_candidates.get(max_tickets - 1).unwrap_or(&TicketId::MAX);

		// Consume at most `max_segments` segments.
		// During the process remove every stale ticket from `TicketsData` storage.
		for _ in 0..max_segments {
			unsorted_segments_count -= 1;
			let segment = UnsortedSegments::<T>::take(unsorted_segments_count);
			metadata.unsorted_tickets_count -= segment.len() as u32;

			// Push only ids with a value less than the current `upper_bound`.
			// As ticket ids follow a uniform random distribution we expect to
			// drop more or less half of the segment tickets here.
			for ticket_id in segment {
				if ticket_id < upper_bound {
					sorted_candidates.push(ticket_id);
				} else {
					TicketsData::<T>::remove(ticket_id);
				}
			}

			if sorted_candidates.len() > max_tickets {
				// Sort, truncate good tickets, cleanup storage.
				require_sort = false;
				sorted_candidates.sort_unstable();
				sorted_candidates[max_tickets..].iter().for_each(TicketsData::<T>::remove);
				sorted_candidates.truncate(max_tickets);
				upper_bound = sorted_candidates[max_tickets - 1];
			}
		}

		if require_sort {
			sorted_candidates.sort_unstable();
		}

		if metadata.unsorted_tickets_count == 0 {
			// Sorting is over, write to next epoch map.
			sorted_candidates.iter().enumerate().for_each(|(i, id)| {
				TicketsIds::<T>::insert((epoch_tag, i as u32), id);
			});
			metadata.tickets_count[epoch_tag as usize] = sorted_candidates.len() as u32;
		} else {
			// Keep the partial result for next calls.
			SortedCandidates::<T>::set(BoundedVec::truncate_from(sorted_candidates));
		}
	}

	/// Append a set of tickets to the segments map.
	pub(crate) fn append_tickets(tickets: BoundedVec<TicketId, MaxTicketsFor<T>>) {
		debug!(target: LOG_TARGET, "Appending batch with {} tickets", tickets.len());
		tickets.iter().for_each(|t| debug!(target: LOG_TARGET, "  + {t:032x}"));

		let mut metadata = TicketsMeta::<T>::get();
		let mut segment_idx = metadata.unsorted_tickets_count / SEGMENT_MAX_SIZE;

		let mut tickets = tickets.as_slice();
		while !tickets.is_empty() {
			let rem = metadata.unsorted_tickets_count % SEGMENT_MAX_SIZE;
			let todo = tickets.len().min((SEGMENT_MAX_SIZE - rem) as usize);

			let mut segment = UnsortedSegments::<T>::get(segment_idx).into_inner();
			segment.extend_from_slice(&tickets[..todo]);
			let segment = BoundedVec::truncate_from(segment);
			UnsortedSegments::<T>::insert(segment_idx, segment);

			metadata.unsorted_tickets_count += todo as u32;
			segment_idx += 1;

			tickets = &tickets[todo..];
		}

		TicketsMeta::<T>::set(metadata);
	}

	/// Remove all tickets related data.
	///
	/// May not be efficient as the calling places may repeat some of this operations
	/// but is a very extraordinary operation (hopefully never happens in production)
	/// and better safe than sorry.
	fn reset_tickets_data() {
		let tickets_metadata = TicketsMeta::<T>::get();

		// Remove even-epoch data.
		let tickets_count = tickets_metadata.tickets_count[0];
		for idx in 0..tickets_count {
			if let Some(id) = TicketsIds::<T>::get((0, idx)) {
				TicketsData::<T>::remove(id);
			}
		}

		// Remove odd-epoch data.
		let tickets_count = tickets_metadata.tickets_count[1];
		for idx in 0..tickets_count {
			if let Some(id) = TicketsIds::<T>::get((1, idx)) {
				TicketsData::<T>::remove(id);
			}
		}

		// Remove all unsorted tickets segments.
		let segments_count = tickets_metadata.unsorted_tickets_count.div_ceil(SEGMENT_MAX_SIZE);
		(0..segments_count).for_each(UnsortedSegments::<T>::remove);

		// Reset sorted candidates
		SortedCandidates::<T>::kill();

		// Reset tickets metadata
		TicketsMeta::<T>::kill();
	}

	/// Submit next epoch validator tickets via an unsigned extrinsic constructed with a call to
	/// `submit_unsigned_transaction`.
	///
	/// The submitted tickets are added to the next epoch outstanding tickets as long as the
	/// extrinsic is called within the first half of the epoch. Tickets received during the
	/// second half are dropped.
	// TODO @davxy: directly use a bounded vector???
	pub fn submit_tickets_unsigned_extrinsic(tickets: Vec<TicketEnvelope>) -> bool {
		let tickets = BoundedVec::truncate_from(tickets);
		let call = Call::submit_tickets { tickets };
		match SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()) {
			Ok(_) => true,
			Err(e) => {
				error!(target: LOG_TARGET, "Error submitting tickets {:?}", e);
				false
			},
		}
	}
}

/// Trigger an epoch change, if any should take place.
pub trait EpochChangeTrigger {
	/// May trigger an epoch change, if any should take place.
	///
	/// Returns an optional `Weight` if epoch change has been triggered.
	///
	/// This should be called during every block, after initialization is done.
	fn trigger<T: Config>(_: BlockNumberFor<T>) -> Option<Weight>;
}

/// An `EpochChangeTrigger` which does nothing.
///
/// In practice this means that the epoch change logic is left to some external component
/// (e.g. pallet-session).
pub struct EpochChangeExternalTrigger;

impl EpochChangeTrigger for EpochChangeExternalTrigger {
	fn trigger<T: Config>(_: BlockNumberFor<T>) -> Option<Weight> {
		// nothing - trigger is external.
		None
	}
}

/// An `EpochChangeTrigger` which recycle the same authorities set forever.
///
/// The internal trigger should only be used when no other module is responsible for
/// changing authority set.
pub struct EpochChangeInternalTrigger;

impl EpochChangeTrigger for EpochChangeInternalTrigger {
	fn trigger<T: Config>(block_num: BlockNumberFor<T>) -> Option<Weight> {
		if Pallet::<T>::should_end_epoch(block_num) {
			let authorities = Pallet::<T>::next_authorities();
			let next_authorities = authorities.clone();
			let len = next_authorities.len() as u32;
			Pallet::<T>::enact_epoch_change(authorities, next_authorities);
			Some(T::WeightInfo::internal_epoch_change_trigger(len))
		} else {
			None
		}
	}
}

impl<T: Config> BoundToRuntimeAppPublic for Pallet<T> {
	type Public = AuthorityId;
}
