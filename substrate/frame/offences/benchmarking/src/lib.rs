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

//! Offences pallet benchmarking.

#![cfg(feature = "runtime-benchmarks")]
#![cfg_attr(not(feature = "std"), no_std)]

mod mock;

use sp_std::{prelude::*, vec};

use frame_benchmarking::v1::{account, benchmarks};
use frame_support::traits::{Currency, Get};
use frame_system::{Config as SystemConfig, Pallet as System, RawOrigin};

#[cfg(test)]
use sp_runtime::traits::UniqueSaturatedInto;
use sp_runtime::{
	traits::{Convert, Saturating, StaticLookup},
	Perbill,
};
use sp_staking::offence::ReportOffence;

use pallet_babe::EquivocationOffence as BabeEquivocationOffence;
use pallet_balances::Config as BalancesConfig;
use pallet_grandpa::{
	EquivocationOffence as GrandpaEquivocationOffence, TimeSlot as GrandpaTimeSlot,
};
use pallet_offences::{Config as OffencesConfig, Pallet as Offences};
use pallet_session::{
	historical::{Config as HistoricalConfig, IdentificationTuple},
	Config as SessionConfig, Pallet as Session, SessionManager,
};
#[cfg(test)]
use pallet_staking::Event as StakingEvent;
use pallet_staking::{
	Config as StakingConfig, Exposure, IndividualExposure, MaxNominationsOf, Pallet as Staking,
	RewardDestination, ValidatorPrefs,
};

const SEED: u32 = 0;

const MAX_NOMINATORS: u32 = 100;

pub struct Pallet<T: Config>(Offences<T>);

pub trait Config:
	SessionConfig
	+ StakingConfig
	+ OffencesConfig
	+ HistoricalConfig
	+ BalancesConfig
	+ IdTupleConvert<Self>
{
}

/// A helper trait to make sure we can convert `IdentificationTuple` coming from historical
/// and the one required by offences.
pub trait IdTupleConvert<T: HistoricalConfig + OffencesConfig> {
	/// Convert identification tuple from `historical` trait to the one expected by `offences`.
	fn convert(id: IdentificationTuple<T>) -> <T as OffencesConfig>::IdentificationTuple;
}

impl<T: HistoricalConfig + OffencesConfig> IdTupleConvert<T> for T
where
	<T as OffencesConfig>::IdentificationTuple: From<IdentificationTuple<T>>,
{
	fn convert(id: IdentificationTuple<T>) -> <T as OffencesConfig>::IdentificationTuple {
		id.into()
	}
}

type LookupSourceOf<T> = <<T as SystemConfig>::Lookup as StaticLookup>::Source;
type BalanceOf<T> =
	<<T as StakingConfig>::Currency as Currency<<T as SystemConfig>::AccountId>>::Balance;

struct Offender<T: Config> {
	pub controller: T::AccountId,
	#[allow(dead_code)]
	pub stash: T::AccountId,
	#[allow(dead_code)]
	pub nominator_stashes: Vec<T::AccountId>,
}

fn bond_amount<T: Config>() -> BalanceOf<T> {
	T::Currency::minimum_balance().saturating_mul(10_000u32.into())
}

fn create_offender<T: Config>(n: u32, nominators: u32) -> Result<Offender<T>, &'static str> {
	let stash: T::AccountId = account("stash", n, SEED);
	let stash_lookup: LookupSourceOf<T> = T::Lookup::unlookup(stash.clone());
	let reward_destination = RewardDestination::Staked;
	let amount = bond_amount::<T>();
	// add twice as much balance to prevent the account from being killed.
	let free_amount = amount.saturating_mul(2u32.into());
	T::Currency::make_free_balance_be(&stash, free_amount);
	Staking::<T>::bond(
		RawOrigin::Signed(stash.clone()).into(),
		amount,
		reward_destination.clone(),
	)?;

	let validator_prefs =
		ValidatorPrefs { commission: Perbill::from_percent(50), ..Default::default() };
	Staking::<T>::validate(RawOrigin::Signed(stash.clone()).into(), validator_prefs)?;

	let mut individual_exposures = vec![];
	let mut nominator_stashes = vec![];
	// Create n nominators
	for i in 0..nominators {
		let nominator_stash: T::AccountId =
			account("nominator stash", n * MAX_NOMINATORS + i, SEED);
		T::Currency::make_free_balance_be(&nominator_stash, free_amount);

		Staking::<T>::bond(
			RawOrigin::Signed(nominator_stash.clone()).into(),
			amount,
			reward_destination.clone(),
		)?;

		let selected_validators: Vec<LookupSourceOf<T>> = vec![stash_lookup.clone()];
		Staking::<T>::nominate(
			RawOrigin::Signed(nominator_stash.clone()).into(),
			selected_validators,
		)?;

		individual_exposures
			.push(IndividualExposure { who: nominator_stash.clone(), value: amount });
		nominator_stashes.push(nominator_stash.clone());
	}

	let exposure = Exposure { total: amount * n.into(), own: amount, others: individual_exposures };
	let current_era = 0u32;
	Staking::<T>::add_era_stakers(current_era, stash.clone(), exposure);

	Ok(Offender { controller: stash.clone(), stash, nominator_stashes })
}

fn make_offenders<T: Config>(
	num_offenders: u32,
	num_nominators: u32,
) -> Result<(Vec<IdentificationTuple<T>>, Vec<Offender<T>>), &'static str> {
	Staking::<T>::new_session(0);

	let mut offenders = vec![];
	for i in 0..num_offenders {
		let offender = create_offender::<T>(i + 1, num_nominators)?;
		offenders.push(offender);
	}

	Staking::<T>::start_session(0);

	let id_tuples = offenders
		.iter()
		.map(|offender| {
			<T as SessionConfig>::ValidatorIdOf::convert(offender.controller.clone())
				.expect("failed to get validator id from account id")
		})
		.map(|validator_id| {
			<T as HistoricalConfig>::FullIdentificationOf::convert(validator_id.clone())
				.map(|full_id| (validator_id, full_id))
				.expect("failed to convert validator id to full identification")
		})
		.collect::<Vec<IdentificationTuple<T>>>();
	Ok((id_tuples, offenders))
}

#[cfg(test)]
fn check_events<
	T: Config,
	I: Iterator<Item = Item>,
	Item: sp_std::borrow::Borrow<<T as SystemConfig>::RuntimeEvent> + sp_std::fmt::Debug,
>(
	expected: I,
) {
	let events = System::<T>::events()
		.into_iter()
		.map(|frame_system::EventRecord { event, .. }| event)
		.collect::<Vec<_>>();
	let expected = expected.collect::<Vec<_>>();

	fn pretty<D: sp_std::fmt::Debug>(header: &str, ev: &[D], offset: usize) {
		log::info!("{}", header);
		for (idx, ev) in ev.iter().enumerate() {
			log::info!("\t[{:04}] {:?}", idx + offset, ev);
		}
	}
	fn print_events<D: sp_std::fmt::Debug, E: sp_std::fmt::Debug>(
		idx: usize,
		events: &[D],
		expected: &[E],
	) {
		let window = 10;
		let start = idx.saturating_sub(window / 2);
		let end_got = (idx + window / 2).min(events.len());
		pretty("Got(window):", &events[start..end_got], start);
		let end_expected = (idx + window / 2).min(expected.len());
		pretty("Expected(window):", &expected[start..end_expected], start);
		log::info!("---------------");
		let start_got = events.len().saturating_sub(window);
		pretty("Got(end):", &events[start_got..], start_got);
		let start_expected = expected.len().saturating_sub(window);
		pretty("Expected(end):", &expected[start_expected..], start_expected);
	}

	for (idx, (a, b)) in events.iter().zip(expected.iter()).enumerate() {
		if a != sp_std::borrow::Borrow::borrow(b) {
			print_events(idx, &events, &expected);
			log::info!("Mismatch at: {}", idx);
			log::info!("     Got: {:?}", b);
			log::info!("Expected: {:?}", a);
			if events.len() != expected.len() {
				log::info!(
					"Mismatching lengths. Got: {}, Expected: {}",
					events.len(),
					expected.len()
				)
			}
			panic!("Mismatching events.");
		}
	}

	if events.len() != expected.len() {
		print_events(0, &events, &expected);
		panic!("Mismatching lengths. Got: {}, Expected: {}", events.len(), expected.len(),)
	}
}

benchmarks! {
	report_offence_grandpa {
		let n in 0 .. MAX_NOMINATORS.min(MaxNominationsOf::<T>::get());

		// for grandpa equivocation reports the number of reporters
		// and offenders is always 1
		let reporters = vec![account("reporter", 1, SEED)];

		// make sure reporters actually get rewarded
		Staking::<T>::set_slash_reward_fraction(Perbill::one());

		let (mut offenders, raw_offenders) = make_offenders::<T>(1, n)?;
		let validator_set_count = Session::<T>::validators().len() as u32;

		let offence = GrandpaEquivocationOffence {
			time_slot: GrandpaTimeSlot { set_id: 0, round: 0 },
			session_index: 0,
			validator_set_count,
			offender: T::convert(offenders.pop().unwrap()),
		};
		assert_eq!(System::<T>::event_count(), 0);
	}: {
		let _ = Offences::<T>::report_offence(reporters, offence);
	}
	verify {
		// make sure that all slashes have been applied
		#[cfg(test)]
		assert_eq!(
			System::<T>::event_count(), 0
			+ 1 // offence
			+ 3 // reporter (reward + endowment)
			+ 1 // offenders reported
			+ 3 // offenders slashed
			+ 1 // offenders chilled
			+ 3 * n // nominators slashed
		);
	}

	report_offence_babe {
		let n in 0 .. MAX_NOMINATORS.min(MaxNominationsOf::<T>::get());

		// for babe equivocation reports the number of reporters
		// and offenders is always 1
		let reporters = vec![account("reporter", 1, SEED)];

		// make sure reporters actually get rewarded
		Staking::<T>::set_slash_reward_fraction(Perbill::one());

		let (mut offenders, raw_offenders) = make_offenders::<T>(1, n)?;
		let validator_set_count = Session::<T>::validators().len() as u32;

		let offence = BabeEquivocationOffence {
			slot: 0u64.into(),
			session_index: 0,
			validator_set_count,
			offender: T::convert(offenders.pop().unwrap()),
		};
		assert_eq!(System::<T>::event_count(), 0);
	}: {
		let _ = Offences::<T>::report_offence(reporters, offence);
	}
	verify {
		// make sure that all slashes have been applied
		#[cfg(test)]
		assert_eq!(
			System::<T>::event_count(), 0
			+ 1 // offence
			+ 3 // reporter (reward + endowment)
			+ 1 // offenders reported
			+ 3 // offenders slashed
			+ 1 // offenders chilled
			+ 3 * n // nominators slashed
		);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
