use crate::{mock::*, Error, LockPeriod, LockPeriods, Owners};
use frame_support::{
	assert_noop, assert_ok,
	traits::{Currency, OnFinalize},
};
use frame_system::Config as SystemConfig;
use pallet_balances::Error as BalancesError;

#[test]
fn straight_forward_commit() {
	new_test_ext().execute_with(|| {
		let alice_id: <Test as SystemConfig>::AccountId = 1;
		let name = b"Alice".to_vec();
		let commit = VanityRegistry::hash_of(alice_id, name);

		let block_number = 7;
		System::set_block_number(block_number.clone());

		assert_ok!(VanityRegistry::commit(Origin::signed(alice_id), commit));

		let lock_period = VanityRegistry::lock_periods(alice_id, commit).unwrap();
		assert_eq!(lock_period.begin, block_number);
		assert_eq!(lock_period.end, block_number + RegisterPeriod::get());
	});
}

#[test]
fn straight_forward_reveal() {
	new_test_ext().execute_with(|| {
		let id: <Test as SystemConfig>::AccountId = 1;
		let name = b"Alice".to_vec();
		let commit = VanityRegistry::hash_of(id.clone(), name.clone());
		assert_ok!(VanityRegistry::commit(Origin::signed(id.clone()), commit.clone()));
		assert_ok!(VanityRegistry::reveal(Origin::signed(id.clone()), name.clone()));

		let owner = VanityRegistry::owners(name).unwrap();
		assert_eq!(owner.id, id);
		assert_eq!(owner.commit, commit);
	});
}

#[test]
fn straight_forward_renew() {
	new_test_ext().execute_with(|| {
		let id: <Test as SystemConfig>::AccountId = 1;
		let name = b"Alice".to_vec();
		let commit = VanityRegistry::hash_of(id.clone(), name.clone());

		System::set_block_number(7);
		assert_ok!(VanityRegistry::commit(Origin::signed(id.clone()), commit.clone()));

		System::set_block_number(8);
		assert_ok!(VanityRegistry::reveal(Origin::signed(id.clone()), name.clone()));

		System::set_block_number(9);
		assert_ok!(VanityRegistry::renew(Origin::signed(id.clone()), name.clone()));

		let new_lock_period = VanityRegistry::owners(name).unwrap().lock_period;
		assert_eq!(new_lock_period, LockPeriod { begin: 7, end: 9 + RegisterPeriod::get() });
	});
}

#[test]
fn straight_forward_unregister() {
	new_test_ext().execute_with(|| {
		let id: <Test as SystemConfig>::AccountId = 1;
		let name = b"Alice".to_vec();
		let commit = VanityRegistry::hash_of(id.clone(), name.clone());

		System::set_block_number(7);
		assert_ok!(VanityRegistry::commit(Origin::signed(id.clone()), commit.clone()));
		assert!(LockPeriods::<Test>::contains_key(id.clone(), commit.clone()));

		System::set_block_number(8);
		assert_ok!(VanityRegistry::reveal(Origin::signed(id.clone()), name.clone()));
		assert!(!LockPeriods::<Test>::contains_key(id.clone(), commit.clone()));
		assert!(Owners::<Test>::contains_key(name.clone()));

		System::set_block_number(9);
		assert_ok!(VanityRegistry::unregister(Origin::signed(id.clone()), name.clone()));

		assert!(!LockPeriods::<Test>::contains_key(id, commit));
		assert!(!Owners::<Test>::contains_key(name));
	});
}

#[test]
fn on_finalize_expired_commits_are_removed() {
	new_test_ext().execute_with(|| {
		let alice_id: <Test as SystemConfig>::AccountId = 1;
		let bob_id: <Test as SystemConfig>::AccountId = 2;
		let dave_id: <Test as SystemConfig>::AccountId = 3;
		let name = b"Alice".to_vec();
		let commit = VanityRegistry::hash_of(alice_id, name);

		System::set_block_number(7);
		assert_ok!(VanityRegistry::commit(Origin::signed(alice_id), commit));

		System::set_block_number(8);
		assert_ok!(VanityRegistry::commit(Origin::signed(bob_id), commit));

		System::set_block_number(9);
		assert_ok!(VanityRegistry::commit(Origin::signed(dave_id), commit));

		assert!(LockPeriods::<Test>::contains_key(alice_id, commit));
		assert!(LockPeriods::<Test>::contains_key(bob_id, commit));
		assert!(LockPeriods::<Test>::contains_key(dave_id, commit));

		VanityRegistry::on_finalize(8 + RegisterPeriod::get());

		assert!(!LockPeriods::<Test>::contains_key(alice_id, commit));
		assert!(!LockPeriods::<Test>::contains_key(bob_id, commit));
		assert!(LockPeriods::<Test>::contains_key(dave_id, commit));
	});
}

#[test]
fn fund_lock_upon_commit() {
	new_test_ext().execute_with(|| {
		let alice_id: <Test as SystemConfig>::AccountId = 1;
		let bob_id: <Test as SystemConfig>::AccountId = 2;
		let name = b"Alice".to_vec();
		let commit = VanityRegistry::hash_of(alice_id, name);

		let alice_balance = FundToLock::get();
		let _ = Balances::deposit_creating(&alice_id, alice_balance.clone());
		assert_eq!(Balances::free_balance(&alice_id), alice_balance);

		assert_ok!(VanityRegistry::commit(Origin::signed(alice_id), commit));
		assert_noop!(
			Balances::transfer(Origin::signed(alice_id), bob_id, 1),
			BalancesError::<Test, _>::LiquidityRestrictions
		);
	});
}

#[test]
fn fund_unlock_upon_unregister() {
	new_test_ext().execute_with(|| {
		let alice_id: <Test as SystemConfig>::AccountId = 1;
		let bob_id: <Test as SystemConfig>::AccountId = 2;
		let name = b"Alice".to_vec();
		let commit = VanityRegistry::hash_of(alice_id, name.clone());

		System::set_block_number(7);
		let alice_balance_no_more_than_lock_amount = FundToLock::get();
		let _ =
			Balances::deposit_creating(&alice_id, alice_balance_no_more_than_lock_amount.clone());
		assert_ok!(VanityRegistry::commit(Origin::signed(alice_id.clone()), commit));

		System::set_block_number(8);
		assert_ok!(VanityRegistry::reveal(Origin::signed(alice_id.clone()), name.clone()));

		System::set_block_number(9);
		assert_ok!(VanityRegistry::unregister(Origin::signed(alice_id.clone()), name.clone()));

		assert_ok!(Balances::transfer(Origin::signed(alice_id), bob_id, 1));
	});
}

#[test]
fn fund_lock_increase_with_more_commits() {
	new_test_ext().execute_with(|| {
		let alice_id: <Test as SystemConfig>::AccountId = 1;
		let bob_id: <Test as SystemConfig>::AccountId = 2;
		let name1 = b"Alice".to_vec();
		let name2 = b"AliceX".to_vec();
		let commit1 = VanityRegistry::hash_of(alice_id, name1);
		let commit2 = VanityRegistry::hash_of(alice_id, name2);

		let alice_balance = 2 * FundToLock::get();
		let _ = Balances::deposit_creating(&alice_id, alice_balance.clone());

		assert_ok!(VanityRegistry::commit(Origin::signed(alice_id), commit1));

		// Alice still has got enough unlocked fund to do the following transfer
		assert_ok!(Balances::transfer(Origin::signed(alice_id), bob_id, 1));

		assert_ok!(VanityRegistry::commit(Origin::signed(alice_id), commit2));

		// But after the second commit all her fund is now locked
		assert_noop!(
			Balances::transfer(Origin::signed(alice_id), bob_id, 1),
			BalancesError::<Test, _>::LiquidityRestrictions
		);
	});
}

#[test]
fn fund_lock_decrease_with_expiry() {
	new_test_ext().execute_with(|| {
		let alice_id: <Test as SystemConfig>::AccountId = 1;
		let bob_id: <Test as SystemConfig>::AccountId = 2;
		let name1 = b"Alice".to_vec();
		let name2 = b"AliceX".to_vec();
		let commit1 = VanityRegistry::hash_of(alice_id, name1);
		let commit2 = VanityRegistry::hash_of(alice_id, name2);

		let alice_balance = 2 * FundToLock::get();
		let _ = Balances::deposit_creating(&alice_id, alice_balance.clone());

		System::set_block_number(7);
		assert_ok!(VanityRegistry::commit(Origin::signed(alice_id), commit1));
		System::set_block_number(8);
		assert_ok!(VanityRegistry::commit(Origin::signed(alice_id), commit2));

		// Alice balance completely locked
		assert_noop!(
			Balances::transfer(Origin::signed(alice_id), bob_id, 1),
			BalancesError::<Test, _>::LiquidityRestrictions
		);

		VanityRegistry::on_finalize(7 + RegisterPeriod::get());

		// Alice balance is partly unlocked
		assert_ok!(Balances::transfer(Origin::signed(alice_id), bob_id, 1));
		assert_noop!(
			Balances::transfer(Origin::signed(alice_id), bob_id, alice_balance - 1),
			BalancesError::<Test, _>::LiquidityRestrictions
		);

		// Alice balance is completely unlocked
		VanityRegistry::on_finalize(8 + RegisterPeriod::get());
		assert_ok!(Balances::transfer(Origin::signed(alice_id), bob_id, alice_balance - 1));
	});
}

#[test]
fn fund_unlock_upon_expiry() {
	new_test_ext().execute_with(|| {
		let alice_id: <Test as SystemConfig>::AccountId = 1;
		let bob_id: <Test as SystemConfig>::AccountId = 2;
		let name = b"Alice".to_vec();
		let commit = VanityRegistry::hash_of(alice_id, name.clone());

		System::set_block_number(7);
		let alice_balance_no_more_than_lock_amount = FundToLock::get();
		let _ =
			Balances::deposit_creating(&alice_id, alice_balance_no_more_than_lock_amount.clone());
		assert_ok!(VanityRegistry::commit(Origin::signed(alice_id.clone()), commit));

		VanityRegistry::on_finalize(7 + RegisterPeriod::get());

		assert_ok!(Balances::transfer(Origin::signed(alice_id), bob_id, 1));
	});
}

#[test]
fn revealing_non_owning_name_fails() {
	new_test_ext().execute_with(|| {
		let alice_id: <Test as SystemConfig>::AccountId = 1;
		let bob_id: <Test as SystemConfig>::AccountId = 2;
		let alice_name = b"Alice".to_vec();
		let alice_commit = VanityRegistry::hash_of(alice_id, alice_name.clone());

		// If Bob wants to pay the price of committing on behalf of Alice, it's ok.
		assert_ok!(VanityRegistry::commit(Origin::signed(bob_id.clone()), alice_commit.clone()));

		// The fact that Bob is the committer, will not help him to take over Alice's name
		assert_noop!(
			VanityRegistry::reveal(Origin::signed(bob_id), alice_name),
			Error::<Test>::CommitNotFound
		);
	});
}

#[test]
fn front_running_is_revertible() {
	new_test_ext().execute_with(|| {
		let alice_id: <Test as SystemConfig>::AccountId = 1;
		let bob_id: <Test as SystemConfig>::AccountId = 2;
		let alice_name = b"Alice".to_vec();
		let alice_commit = VanityRegistry::hash_of(alice_id, alice_name.clone());

		System::set_block_number(1);
		assert_ok!(VanityRegistry::commit(Origin::signed(alice_id.clone()), alice_commit.clone()));

		System::set_block_number(2);
		let bob_commit_for_alice_name = VanityRegistry::hash_of(bob_id.clone(), alice_name.clone());
		assert_ok!(VanityRegistry::commit(
			Origin::signed(bob_id.clone()),
			bob_commit_for_alice_name.clone()
		));

		// Bob can temporarily claim over alice name
		assert_ok!(VanityRegistry::reveal(Origin::signed(bob_id), alice_name.clone()));

		// Alice can revert Bob's claim
		assert_ok!(VanityRegistry::reveal(Origin::signed(alice_id), alice_name.clone()));
		let owner = VanityRegistry::owners(alice_name).unwrap();
		assert_eq!(owner.commit, alice_commit);
		assert_eq!(owner.id, alice_id);
	});
}

#[test]
fn revealing_an_already_taken_name_fails() {
	new_test_ext().execute_with(|| {
		let alice_id: <Test as SystemConfig>::AccountId = 1;
		let bob_id: <Test as SystemConfig>::AccountId = 2;
		let alice_name = b"Alice".to_vec();
		let alice_commit = VanityRegistry::hash_of(alice_id, alice_name.clone());

		System::set_block_number(1);
		assert_ok!(VanityRegistry::commit(Origin::signed(alice_id.clone()), alice_commit.clone()));

		System::set_block_number(2);
		assert_ok!(VanityRegistry::reveal(Origin::signed(alice_id), alice_name.clone()));

		System::set_block_number(3);
		let bob_commit_for_alice_name = VanityRegistry::hash_of(bob_id.clone(), alice_name.clone());
		assert_ok!(VanityRegistry::commit(
			Origin::signed(bob_id.clone()),
			bob_commit_for_alice_name.clone()
		));

		System::set_block_number(4);
		assert_ok!(VanityRegistry::reveal(Origin::signed(bob_id), alice_name.clone()));
	});
}
