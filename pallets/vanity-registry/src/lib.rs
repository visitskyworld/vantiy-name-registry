#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;
pub use weights::WeightInfo;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;

use codec::{Decode, Encode, EncodeLike};
use scale_info::TypeInfo;

use frame_support::traits::{Currency, Get, LockIdentifier, LockableCurrency, WithdrawReasons};
use frame_system::ensure_signed;
use sp_runtime::{
	traits::{Hash, Saturating},
	SaturatedConversion,
};
use sp_std::{fmt::Debug, vec::Vec};

/// The period during which a fund for a commit will be locked
#[derive(Decode, Encode, Clone, Eq, PartialEq, Debug, Default, TypeInfo)]
pub struct LockPeriod<BlockNumber> {
	begin: BlockNumber,
	end: BlockNumber,
}

/// An account with a commit
#[derive(Decode, Encode, Clone, Eq, PartialEq, Default, TypeInfo)]
pub struct Owner<AccountId, Hash, BlockNumber> {
	id: AccountId,
	commit: Hash,
	lock_period: LockPeriod<BlockNumber>,
}

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type OwnerOf<T> = Owner<
	<T as frame_system::Config>::AccountId,
	<T as frame_system::Config>::Hash,
	<T as frame_system::Config>::BlockNumber,
>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
	use frame_system::pallet_prelude::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The currency that people use to lock their funds in, when they register.
		type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;

		/// The type of the names which are the main assets of this module.
		type Name: EncodeLike + Clone + Decode + Eq + PartialEq + Debug + TypeInfo;

		/// Identifier for the pallet's locks
		#[pallet::constant]
		type ModuleId: Get<LockIdentifier>;

		/// A name is kept registered for a certain period configured in the runtime.
		#[pallet::constant]
		type RegisterPeriod: Get<Self::BlockNumber>;

		/// A fund should be locked as long as the name is kept for an account.
		#[pallet::constant]
		type FundToLock: Get<BalanceOf<Self>>;

		#[pallet::constant]
		type NameMaxLen: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// The lock periods mapped to their corresponding account ids and commits
	#[pallet::storage]
	#[pallet::getter(fn lock_periods)]
	pub(super) type LockPeriods<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Identity,
		T::Hash,
		LockPeriod<T::BlockNumber>,
	>;

	/// Owners (account id + commit) mapped to their revealed names
	#[pallet::storage]
	#[pallet::getter(fn owners)]
	pub(super) type Owners<T: Config> = StorageMap<_, Blake2_128Concat, T::Name, OwnerOf<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Register of a name for an AccountId succeeded.
		/// The third field will be Some(id) if this name was deemed to belong to that "id" prior to this event.
		NameOwned(T::Name, T::AccountId),
		/// The name is freed, either got expired from someone's possession or unregistered.
		NameFreed(T::Name),
		/// There has been a claim just discovered which wins over this claim. The claimer's fund will be unlocked.
		RevealDiscredited(T::Name, T::AccountId),
		/// The claim got expired before being able to register a name.
		CommitExpired(T::Hash, T::AccountId),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The expected name is not registered at all.
		NameNotFound,
		/// Name is not registered for the requester before.
		NameNotOwned,
		/// The hash_of(account_id + name) must have been provided before a reveal.
		CommitNotFound,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		/// Find and remove expired commits and free the corresponding currency locks at block n.
		fn on_finalize(n: T::BlockNumber) {
			Self::remove_expired_commits(n);
			Self::remove_expired_names(n);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Commit the hash of 'your id concatenated to your desired name'.
		/// Reveal the name only after you made sure your commit is registered.
		#[pallet::weight(T::WeightInfo::commit())]
		pub fn commit(origin: OriginFor<T>, hash: T::Hash) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let begin = <frame_system::Pallet<T>>::block_number();
			let end = begin + T::RegisterPeriod::get();
			let lock_period: LockPeriod<T::BlockNumber> = LockPeriod { begin, end };
			<LockPeriods<T>>::insert(who.clone(), hash, lock_period);
			Self::update_locked_fund(who);
			Ok(())
		}

		/// Reveal the name for which you have previously registered a commit.
		#[pallet::weight(T::WeightInfo::reveal(name.encode().len()))]
		pub fn reveal(origin: OriginFor<T>, name: T::Name) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let commit = Self::hash_of(who.clone(), name.clone());

			let new_claim_lock_period =
				LockPeriods::<T>::take(who.clone(), commit).ok_or(Error::<T>::CommitNotFound)?;

			if let Some(current_owner) = Owners::<T>::get(name.clone()) {
				if current_owner.lock_period.begin <= new_claim_lock_period.begin {
					Self::update_locked_fund(who.clone());
					Self::deposit_event(Event::RevealDiscredited(name, who));
					return Ok(()); // The reveal originator has successfully discredited their own reveal!
				};
			}

			// TODO check if mutate is necessary
			Owners::<T>::insert(
				name.clone(),
				Owner { id: who.clone(), commit, lock_period: new_claim_lock_period },
			);

			Self::deposit_event(Event::NameOwned(name, who));

			Ok(())
		}

		/// Renew the "name" for "origin". The name should belong to "origin" in the first place.
		/// When successful, this will extend the register period by another "RegisterPeriod" since
		/// the renew time.
		#[pallet::weight(T::WeightInfo::renew())]
		pub fn renew(origin: OriginFor<T>, name: T::Name) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let mut owner = Self::ensure_owner(who, name.clone())?;

			owner.lock_period.end =
				<frame_system::Pallet<T>>::block_number() + T::RegisterPeriod::get();
			Owners::<T>::insert(name, owner);

			Ok(())
		}

		/// Unregister the name for origin and unlock the associated fund
		#[pallet::weight(T::WeightInfo::unregister())]
		pub fn unregister(origin: OriginFor<T>, name: T::Name) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let _ = Self::ensure_owner(who.clone(), name.clone())?;

			Owners::<T>::remove(name.clone());

			Self::update_locked_fund(who);

			Self::deposit_event(Event::NameFreed(name));

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Set lock according to the number of commits that are associated with and id.
	/// Remove the lock if no commits.
	fn update_locked_fund(id: T::AccountId) {
		let num_of_commits = LockPeriods::<T>::iter_prefix_values(id.clone()).count();
		if num_of_commits > 0 {
			let amount_to_lock =
				T::FundToLock::get().saturating_mul(num_of_commits.saturated_into());
			T::Currency::set_lock(T::ModuleId::get(), &id, amount_to_lock, WithdrawReasons::all());
		} else {
			T::Currency::remove_lock(T::ModuleId::get(), &id);
		}
	}

	/// Remove expired commits for which the lock period is over.
	fn remove_expired_commits(now: T::BlockNumber) {
		let expired_commits: Vec<(T::AccountId, T::Hash)> = LockPeriods::<T>::iter()
			.filter(|(_, _, lock_period)| lock_period.end <= now)
			.map(|(id, commit, _)| (id, commit))
			.collect();
		expired_commits.iter().for_each(|(id, commit)| {
			LockPeriods::<T>::remove(id.clone(), commit);
			Self::update_locked_fund(id.clone());
			Self::deposit_event(Event::CommitExpired(*commit, id.clone()));
		});
	}

	/// Free names when their corresponding fund lock is expired.
	fn remove_expired_names(now: T::BlockNumber) {
		let expired_names: Vec<(T::Name, OwnerOf<T>)> =
			Owners::<T>::iter().filter(|(_, owner)| owner.lock_period.end <= now).collect();
		expired_names.iter().for_each(|(name, owner)| {
			Owners::<T>::remove(name.clone());
			Self::update_locked_fund(owner.id.clone());
			Self::deposit_event(Event::NameFreed(name.clone()));
		});
	}

	/// Calculate the commit for "name" from "id" which the hash of 'id concatenated name'.
	fn hash_of(id: T::AccountId, name: T::Name) -> T::Hash {
		let mut id_plus_name = id.encode();
		id_plus_name.extend_from_slice(&name.encode());
		T::Hashing::hash_of(&id_plus_name)
	}

	/// Ensure origin is the owner of the "name" and when successful return the ownership details.
	fn ensure_owner(origin: T::AccountId, name: T::Name) -> Result<OwnerOf<T>, Error<T>> {
		if let Some(owner) = Owners::<T>::get(name) {
			if owner.id != origin {
				Err(Error::<T>::NameNotOwned)
			} else {
				Ok(owner)
			}
		} else {
			Err(Error::<T>::NameNotFound)
		}
	}
}
