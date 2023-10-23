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

//! Autogenerated weights for `pallet_sassafras`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-10-20, STEPS: `10`, REPEAT: `3`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `behemoth`, CPU: `AMD Ryzen Threadripper 3970X 32-Core Processor`
//! WASM-EXECUTION: `Compiled`, CHAIN: `Some("dev")`, DB CACHE: `1024`

// Executed Command:
// ./target/release/node-template
// benchmark
// pallet
// --chain
// dev
// --pallet
// pallet_sassafras
// --extrinsic
// *
// --steps
// 10
// --repeat
// 3
// --output
// weights.rs
// --template
// substrate/.maintain/frame-weight-template.hbs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for `pallet_sassafras`.
pub trait WeightInfo {
	fn submit_tickets(x: u32, ) -> Weight;
	fn plan_config_change() -> Weight;
	fn load_ring_context() -> Weight;
	fn update_ring_verifier(x: u32, ) -> Weight;
	fn sort_segments(x: u32, ) -> Weight;
}

/// Weights for `pallet_sassafras` using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: `Sassafras::RingVerifierData` (r:1 w:0)
	/// Proof: `Sassafras::RingVerifierData` (`max_values`: Some(1), `max_size`: Some(388), added: 883, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::NextAuthorities` (r:1 w:0)
	/// Proof: `Sassafras::NextAuthorities` (`max_values`: Some(1), `max_size`: Some(331), added: 826, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::NextEpochConfig` (r:1 w:0)
	/// Proof: `Sassafras::NextEpochConfig` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::NextRandomness` (r:1 w:0)
	/// Proof: `Sassafras::NextRandomness` (`max_values`: Some(1), `max_size`: Some(32), added: 527, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::EpochIndex` (r:1 w:0)
	/// Proof: `Sassafras::EpochIndex` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::TicketsData` (r:20 w:20)
	/// Proof: `Sassafras::TicketsData` (`max_values`: None, `max_size`: Some(84), added: 2559, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::TicketsMeta` (r:1 w:1)
	/// Proof: `Sassafras::TicketsMeta` (`max_values`: Some(1), `max_size`: Some(12), added: 507, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::NextTicketsSegments` (r:1 w:1)
	/// Proof: `Sassafras::NextTicketsSegments` (`max_values`: None, `max_size`: Some(2054), added: 4529, mode: `MaxEncodedLen`)
	/// The range of component `x` is `[1, 20]`.
	fn submit_tickets(x: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1228`
		//  Estimated: `5519 + x * (2559 ±0)`
		// Minimum execution time: 24_623_980_000 picoseconds.
		Weight::from_parts(12_599_273_288, 5519)
			// Standard Error: 28_081_674
			.saturating_add(Weight::from_parts(11_628_949_482, 0).saturating_mul(x.into()))
			.saturating_add(T::DbWeight::get().reads(7_u64))
			.saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(x.into())))
			.saturating_add(T::DbWeight::get().writes(2_u64))
			.saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(x.into())))
			.saturating_add(Weight::from_parts(0, 2559).saturating_mul(x.into()))
	}
	/// Storage: `Sassafras::PendingEpochConfigChange` (r:0 w:1)
	/// Proof: `Sassafras::PendingEpochConfigChange` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	fn plan_config_change() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 4_158_000 picoseconds.
		Weight::from_parts(4_519_000, 0)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: `Sassafras::RingContext` (r:1 w:0)
	/// Proof: `Sassafras::RingContext` (`max_values`: Some(1), `max_size`: Some(295412), added: 295907, mode: `MaxEncodedLen`)
	fn load_ring_context() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `295540`
		//  Estimated: `296897`
		// Minimum execution time: 20_987_803_000 picoseconds.
		Weight::from_parts(21_097_836_000, 296897)
			.saturating_add(T::DbWeight::get().reads(1_u64))
	}
	/// Storage: `Sassafras::RingContext` (r:1 w:0)
	/// Proof: `Sassafras::RingContext` (`max_values`: Some(1), `max_size`: Some(295412), added: 295907, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::RingVerifierData` (r:0 w:1)
	/// Proof: `Sassafras::RingVerifierData` (`max_values`: Some(1), `max_size`: Some(388), added: 883, mode: `MaxEncodedLen`)
	/// The range of component `x` is `[1, 20]`.
	fn update_ring_verifier(x: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `295540`
		//  Estimated: `296897`
		// Minimum execution time: 52_287_115_000 picoseconds.
		Weight::from_parts(55_034_613_693, 296897)
			// Standard Error: 48_024_911
			.saturating_add(Weight::from_parts(10_369_531, 0).saturating_mul(x.into()))
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: `Sassafras::NextTicketsSegments` (r:101 w:100)
	/// Proof: `Sassafras::NextTicketsSegments` (`max_values`: None, `max_size`: Some(2054), added: 4529, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::TicketsIds` (r:0 w:3600)
	/// Proof: `Sassafras::TicketsIds` (`max_values`: None, `max_size`: Some(21), added: 2496, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::TicketsData` (r:0 w:9200)
	/// Proof: `Sassafras::TicketsData` (`max_values`: None, `max_size`: Some(84), added: 2559, mode: `MaxEncodedLen`)
	/// The range of component `x` is `[1, 100]`.
	fn sort_segments(x: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `219 + x * (2060 ±0)`
		//  Estimated: `5519 + x * (4529 ±0)`
		// Minimum execution time: 189_333_000 picoseconds.
		Weight::from_parts(189_333_000, 5519)
			// Standard Error: 3_306_712
			.saturating_add(Weight::from_parts(256_199_560, 0).saturating_mul(x.into()))
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(x.into())))
			.saturating_add(T::DbWeight::get().writes((129_u64).saturating_mul(x.into())))
			.saturating_add(Weight::from_parts(0, 4529).saturating_mul(x.into()))
	}
}

// For backwards compatibility and tests.
impl WeightInfo for () {
	/// Storage: `Sassafras::RingVerifierData` (r:1 w:0)
	/// Proof: `Sassafras::RingVerifierData` (`max_values`: Some(1), `max_size`: Some(388), added: 883, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::NextAuthorities` (r:1 w:0)
	/// Proof: `Sassafras::NextAuthorities` (`max_values`: Some(1), `max_size`: Some(331), added: 826, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::NextEpochConfig` (r:1 w:0)
	/// Proof: `Sassafras::NextEpochConfig` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::NextRandomness` (r:1 w:0)
	/// Proof: `Sassafras::NextRandomness` (`max_values`: Some(1), `max_size`: Some(32), added: 527, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::EpochIndex` (r:1 w:0)
	/// Proof: `Sassafras::EpochIndex` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::TicketsData` (r:20 w:20)
	/// Proof: `Sassafras::TicketsData` (`max_values`: None, `max_size`: Some(84), added: 2559, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::TicketsMeta` (r:1 w:1)
	/// Proof: `Sassafras::TicketsMeta` (`max_values`: Some(1), `max_size`: Some(12), added: 507, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::NextTicketsSegments` (r:1 w:1)
	/// Proof: `Sassafras::NextTicketsSegments` (`max_values`: None, `max_size`: Some(2054), added: 4529, mode: `MaxEncodedLen`)
	/// The range of component `x` is `[1, 20]`.
	fn submit_tickets(x: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1228`
		//  Estimated: `5519 + x * (2559 ±0)`
		// Minimum execution time: 24_623_980_000 picoseconds.
		Weight::from_parts(12_599_273_288, 5519)
			// Standard Error: 28_081_674
			.saturating_add(Weight::from_parts(11_628_949_482, 0).saturating_mul(x.into()))
			.saturating_add(RocksDbWeight::get().reads(7_u64))
			.saturating_add(RocksDbWeight::get().reads((1_u64).saturating_mul(x.into())))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
			.saturating_add(RocksDbWeight::get().writes((1_u64).saturating_mul(x.into())))
			.saturating_add(Weight::from_parts(0, 2559).saturating_mul(x.into()))
	}
	/// Storage: `Sassafras::PendingEpochConfigChange` (r:0 w:1)
	/// Proof: `Sassafras::PendingEpochConfigChange` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	fn plan_config_change() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 4_158_000 picoseconds.
		Weight::from_parts(4_519_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `Sassafras::RingContext` (r:1 w:0)
	/// Proof: `Sassafras::RingContext` (`max_values`: Some(1), `max_size`: Some(295412), added: 295907, mode: `MaxEncodedLen`)
	fn load_ring_context() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `295540`
		//  Estimated: `296897`
		// Minimum execution time: 20_987_803_000 picoseconds.
		Weight::from_parts(21_097_836_000, 296897)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
	}
	/// Storage: `Sassafras::RingContext` (r:1 w:0)
	/// Proof: `Sassafras::RingContext` (`max_values`: Some(1), `max_size`: Some(295412), added: 295907, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::RingVerifierData` (r:0 w:1)
	/// Proof: `Sassafras::RingVerifierData` (`max_values`: Some(1), `max_size`: Some(388), added: 883, mode: `MaxEncodedLen`)
	/// The range of component `x` is `[1, 20]`.
	fn update_ring_verifier(x: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `295540`
		//  Estimated: `296897`
		// Minimum execution time: 52_287_115_000 picoseconds.
		Weight::from_parts(55_034_613_693, 296897)
			// Standard Error: 48_024_911
			.saturating_add(Weight::from_parts(10_369_531, 0).saturating_mul(x.into()))
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `Sassafras::NextTicketsSegments` (r:101 w:100)
	/// Proof: `Sassafras::NextTicketsSegments` (`max_values`: None, `max_size`: Some(2054), added: 4529, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::TicketsIds` (r:0 w:3600)
	/// Proof: `Sassafras::TicketsIds` (`max_values`: None, `max_size`: Some(21), added: 2496, mode: `MaxEncodedLen`)
	/// Storage: `Sassafras::TicketsData` (r:0 w:9200)
	/// Proof: `Sassafras::TicketsData` (`max_values`: None, `max_size`: Some(84), added: 2559, mode: `MaxEncodedLen`)
	/// The range of component `x` is `[1, 100]`.
	fn sort_segments(x: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `219 + x * (2060 ±0)`
		//  Estimated: `5519 + x * (4529 ±0)`
		// Minimum execution time: 189_333_000 picoseconds.
		Weight::from_parts(189_333_000, 5519)
			// Standard Error: 3_306_712
			.saturating_add(Weight::from_parts(256_199_560, 0).saturating_mul(x.into()))
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().reads((1_u64).saturating_mul(x.into())))
			.saturating_add(RocksDbWeight::get().writes((129_u64).saturating_mul(x.into())))
			.saturating_add(Weight::from_parts(0, 4529).saturating_mul(x.into()))
	}
}
