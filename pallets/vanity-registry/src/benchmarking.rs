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

use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;

use crate::Module as VanityRegistry;
use frame_system::Module as System;
use sp_std::{boxed::Box, vec};

benchmarks! {
    _{ }

    commit {
        let alice_id: T::AccountId = whitelisted_caller();
        const ABCDE: [u8; 5] = [04, 66, 67, 68, 69];
        let name: T::Name = Decode::decode(&mut &ABCDE[..]).unwrap();
        let c = VanityRegistry::<T>::hash_of(alice_id.clone(), name);

        let block_number: T::BlockNumber = (1u32).into();
        System::<T>::set_block_number(block_number.clone());
    }: commit(RawOrigin::Signed(alice_id.clone()), c.clone())
    verify {
        let lock_period = VanityRegistry::<T>::lock_periods(alice_id, c);
        assert_eq!(lock_period.begin, block_number);
        assert_eq!(lock_period.end, block_number + T::RegisterPeriod::get());
    }

    reveal {
        let alice_id: T::AccountId = whitelisted_caller();
        let bob_id: T::AccountId = account("bob", 0, 0);
        const ABCDE: [u8; 5] = [04, 66, 67, 68, 69];
        let alice_name: T::Name = Decode::decode(&mut &ABCDE[..]).unwrap();
        let alice_commit = VanityRegistry::<T>::hash_of(alice_id.clone(), alice_name.clone());

        System::<T>::set_block_number((1u32).into());
        let _ = VanityRegistry::<T>::commit(
            RawOrigin::Signed(alice_id.clone()).into(),
            alice_commit.clone()
        );

        System::<T>::set_block_number((2u32).into());
        let bob_commit_for_alice_name = VanityRegistry::<T>::hash_of(bob_id.clone(), alice_name.clone());
        let _ = VanityRegistry::<T>::commit(
            RawOrigin::Signed(bob_id.clone()).into(),
            bob_commit_for_alice_name.clone()
        );

        // Bob can temporarily claim over alice name
        let _ = VanityRegistry::<T>::reveal(RawOrigin::Signed(bob_id).into(), alice_name.clone());
    }: reveal(RawOrigin::Signed(alice_id.clone()), alice_name.clone())
    verify {
        let committer = VanityRegistry::<T>::committers(alice_name);
        assert_eq!(committer.commit, alice_commit);
        assert_eq!(committer.id, alice_id);
    }

    renew {
        let id: T::AccountId = whitelisted_caller();
        const ABCDE: [u8; 5] = [04, 66, 67, 68, 69];
        let name: T::Name = Decode::decode(&mut &ABCDE[..]).unwrap();
        let c = VanityRegistry::<T>::hash_of(id.clone(), name.clone());

        System::<T>::set_block_number((7u32).into());
        let _ = VanityRegistry::<T>::commit(RawOrigin::Signed(id.clone()).into(), c.clone());

        System::<T>::set_block_number((8u32).into());
        let _ = VanityRegistry::<T>::reveal(RawOrigin::Signed(id.clone()).into(), name.clone());

        System::<T>::set_block_number((9u32).into());
    }: renew(RawOrigin::Signed(id.clone()), name.clone())
    verify {
        let lock_period = VanityRegistry::<T>::lock_periods(id, c);
        assert_eq!(lock_period.end, T::BlockNumber::from(9u32) + T::RegisterPeriod::get());
    }

    unregister {
        let id: T::AccountId = whitelisted_caller();
        const ABCDE: [u8; 5] = [04, 66, 67, 68, 69];
        let name: T::Name = Decode::decode(&mut &ABCDE[..]).unwrap();
        let c = VanityRegistry::<T>::hash_of(id.clone(), name.clone());

        System::<T>::set_block_number((7u32).into());
        let _ = VanityRegistry::<T>::commit(RawOrigin::Signed(id.clone()).into(), c.clone());

        System::<T>::set_block_number((8u32).into());
        let _ = VanityRegistry::<T>::reveal(RawOrigin::Signed(id.clone()).into(), name.clone());

        assert!(LockPeriods::<T>::contains_key(
            id.clone(),
            c.clone()
        ));
        assert!(Committers::<T>::contains_key(name.clone()));

        System::<T>::set_block_number((9u32).into());
    }: unregister(RawOrigin::Signed(id.clone()), name.clone())
    verify {
        assert!(!LockPeriods::<T>::contains_key(id, c));
        assert!(!Committers::<T>::contains_key(name));
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
