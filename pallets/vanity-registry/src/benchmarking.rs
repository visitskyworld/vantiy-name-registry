// This file is part of Vanity Registry.

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

//! Vanity Registry benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{account, benchmarks, vec, whitelisted_caller};
use frame_system::RawOrigin as SystemOrigin;

use crate::Pallet as VanityRegistry;
use frame_system::Pallet as System;

fn create_name<T: Config>(len: u32) -> T::Name {
	// TODO for a better benchmarking we can create random chunks to evade a potential storage compression
	let raw = vec![66u8; len as usize];
	let encoded = raw.encode();
	Decode::decode(&mut encoded.as_slice()).unwrap()
}

benchmarks! {

	commit {
		let alice_id: T::AccountId = whitelisted_caller();
		const ABCDE: [u8; 5] = [04, 66, 67, 68, 69];
		let name: T::Name = Decode::decode(&mut &ABCDE[..]).unwrap();
		let c = VanityRegistry::<T>::hash_of(alice_id.clone(), name);

		let block_number: T::BlockNumber = (1u32).into();
		System::<T>::set_block_number(block_number.clone());
	}: commit(SystemOrigin::Signed(alice_id.clone()), c.clone())
	verify {
		let lock_period = VanityRegistry::<T>::lock_periods(alice_id, c).unwrap();
		assert_eq!(lock_period.begin, block_number);
		assert_eq!(lock_period.end, block_number + T::RegisterPeriod::get());
	}

	reveal {
		let l in 0..T::NameMaxLen::get();
		let alice_id: T::AccountId = whitelisted_caller();
		let bob_id: T::AccountId = account("bob", 0, 0);
		let alice_name = create_name::<T>(l as u32);
		let alice_commit = VanityRegistry::<T>::hash_of(alice_id.clone(), alice_name.clone());

		System::<T>::set_block_number((1u32).into());
		let _ = VanityRegistry::<T>::commit(
			SystemOrigin::Signed(alice_id.clone()).into(),
			alice_commit.clone()
		);

		System::<T>::set_block_number((2u32).into());
		let bob_commit_for_alice_name = VanityRegistry::<T>::hash_of(bob_id.clone(), alice_name.clone());
		let _ = VanityRegistry::<T>::commit(
			SystemOrigin::Signed(bob_id.clone()).into(),
			bob_commit_for_alice_name.clone()
		);

		// Bob can temporarily claim over alice name
		let _ = VanityRegistry::<T>::reveal(SystemOrigin::Signed(bob_id).into(), alice_name.clone());
	}: reveal(SystemOrigin::Signed(alice_id.clone()), alice_name.clone())
	verify {
		let owner = VanityRegistry::<T>::owners(alice_name).unwrap();
		assert_eq!(owner.commit, alice_commit);
		assert_eq!(owner.id, alice_id);
	}

	renew {
		let id: T::AccountId = whitelisted_caller();
		const ABCDE: [u8; 5] = [04, 66, 67, 68, 69];
		let name: T::Name = Decode::decode(&mut &ABCDE[..]).unwrap();
		let c = VanityRegistry::<T>::hash_of(id.clone(), name.clone());

		System::<T>::set_block_number((7u32).into());
		let _ = VanityRegistry::<T>::commit(SystemOrigin::Signed(id.clone()).into(), c.clone());

		System::<T>::set_block_number((8u32).into());
		let _ = VanityRegistry::<T>::reveal(SystemOrigin::Signed(id.clone()).into(), name.clone());

		System::<T>::set_block_number((9u32).into());
	}: renew(SystemOrigin::Signed(id.clone()), name.clone())
	verify {
		let lock_period = VanityRegistry::<T>::owners(name).unwrap().lock_period;
		assert_eq!(lock_period.end, T::BlockNumber::from(9u32) + T::RegisterPeriod::get());
	}

	unregister {
		let id: T::AccountId = whitelisted_caller();
		const ABCDE: [u8; 5] = [04, 66, 67, 68, 69];
		let name: T::Name = Decode::decode(&mut &ABCDE[..]).unwrap();
		let c = VanityRegistry::<T>::hash_of(id.clone(), name.clone());

		System::<T>::set_block_number((7u32).into());
		let _ = VanityRegistry::<T>::commit(SystemOrigin::Signed(id.clone()).into(), c.clone());

		System::<T>::set_block_number((8u32).into());
		let _ = VanityRegistry::<T>::reveal(SystemOrigin::Signed(id.clone()).into(), name.clone());

		assert!(Owners::<T>::contains_key(name.clone()));

		System::<T>::set_block_number((9u32).into());
	}: unregister(SystemOrigin::Signed(id.clone()), name.clone())
	verify {
		assert!(!LockPeriods::<T>::contains_key(id, c));
		assert!(!Owners::<T>::contains_key(name));
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{new_test_ext, Test};
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_commit::<Test>());
			assert_ok!(test_benchmark_reveal::<Test>());
			assert_ok!(test_benchmark_renew::<Test>());
			assert_ok!(test_benchmark_unregister::<Test>());
		});
	}
}
