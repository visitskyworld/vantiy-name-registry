#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, EncodeLike};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch, ensure,
    traits::{Currency, Get, LockIdentifier, LockableCurrency, WithdrawReasons},
    weights::Weight,
    IterableStorageMap,
};
use frame_system::ensure_signed;
use sp_runtime::{traits::Hash, SaturatedConversion};
use sp_std::{fmt::Debug, vec::Vec};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod default_weights;

#[derive(Decode, Encode, Clone, Eq, PartialEq, Debug)]
/// The period during which a fund for a commit will be locked
pub struct LockPeriod<T: frame_system::Trait> {
    begin: T::BlockNumber,
    end: T::BlockNumber,
}
impl<T: frame_system::Trait> Default for LockPeriod<T> {
    fn default() -> Self {
        Self {
            begin: T::BlockNumber::default(),
            end: T::BlockNumber::default(),
        }
    }
}

#[derive(Decode, Encode, Clone, Eq, PartialEq)]
/// Aan account with a commit
pub struct Committer<T: frame_system::Trait> {
    id: T::AccountId,
    commit: T::Hash,
}
impl<T: frame_system::Trait> Default for Committer<T> {
    fn default() -> Self {
        Self {
            id: T::AccountId::default(),
            commit: T::Hash::default(),
        }
    }
}

#[derive(Decode, Encode)]
pub struct LockPeriodOption<T: frame_system::Trait>(Option<LockPeriod<T>>);

impl<T: frame_system::Trait> Default for LockPeriodOption<T> {
    fn default() -> Self {
        Self(None)
    }
}

type BalanceOf<T> =
    <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

pub trait WeightInfo {
    fn commit() -> Weight;
    fn reveal(name_length: usize) -> Weight;
    fn renew() -> Weight;
    fn unregister() -> Weight;
}

pub trait Trait: frame_system::Trait {
    /// This pallet declares its own events.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    /// The currency that people use to lock their funds in, when they register.
    type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
    /// Identifier for the elections pallet's lock
    type ModuleId: Get<LockIdentifier>;
    /// A name is kept registered for a certain period configured in the runtime.
    type RegisterPeriod: Get<Self::BlockNumber>;
    /// A fund should be locked as long as the name is kept for an account.
    type FundToLock: Get<BalanceOf<Self>>;
    /// The type of the names which are the main assets of this module.
    type Name: EncodeLike + Clone + Decode + Eq + PartialEq + Debug;
    /// Weight information for extrinsics in this pallet.
    type WeightInfo: WeightInfo;
}

decl_storage! {
    trait Store for Module<T: Trait> as VanityRegistry {
        /// The lock periods mapped to their corresponding account ids and commits
        LockPeriods get(fn lock_periods): double_map hasher(blake2_128_concat) T::AccountId, hasher(identity) T::Hash => LockPeriod<T>;
        /// Committers (account id + commit) mapped to their revealed names
        Committers get(fn committers): map hasher(blake2_128_concat) T::Name => Committer<T>;
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
        Name = <T as Trait>::Name,
    {
        /// Register of a name for an AccountId succeeded.
        /// The third field will be Some(id) if this name was deemed to belong to that "id" prior to this event.
        RegisterSucceeded(AccountId, Name, Option<AccountId>),
        /// The name is freed, either got expired from someone's possession or unregistered.
        NameFreed(Name),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// The expected name is not registered at all.
        NameNotFound,
        /// The name is already registered for another account.
        NameAlreadyTaken,
        /// Name is not registered for the requester before.
        NameNotOwned,
        /// The hash_of(account_id + name) must have been provided before a reveal.
        CommitNotFound,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        #[weight = T::WeightInfo::commit()]
        /// Commit the hash of 'your id concatenated to your desired name'.
        /// Reveal the name only after you made sure your commit is registered.
        pub fn commit(origin, hash: T::Hash) -> dispatch::DispatchResult {
            let who = ensure_signed(origin)?;
            let begin = <frame_system::Module<T>>::block_number();
            let end = begin.clone() + T::RegisterPeriod::get();
            let lock_period: LockPeriod<T> = LockPeriod {begin, end};
            <LockPeriods<T>>::insert(who.clone(), hash.clone(), lock_period);
            Self::update_locked_fund(who);
            Ok(())
        }

        #[weight = T::WeightInfo::reveal(name.encode().len())]
        /// Reveal the name for which you have previously registered a commit.
        pub fn reveal(origin, name: T::Name) -> dispatch::DispatchResult {
            let who = ensure_signed(origin)?;

            let commit = Self::hash_of(who.clone(), name.clone());
            ensure!(LockPeriods::<T>::contains_key(who.clone(), commit.clone()), Error::<T>::CommitNotFound);

            if !Committers::<T>::contains_key(name.clone()) {
                Committers::<T>::insert(name.clone(), Committer{id: who.clone(), commit});
                Self::deposit_event(RawEvent::RegisterSucceeded(who, name, None));
                return Ok(());
            }

            let revealer_begin = LockPeriods::<T>::get(who.clone(), commit.clone()).begin.clone();

            let current_committer = Committers::<T>::get(name.clone());
            let current_begin = LockPeriods::<T>::get(current_committer.id.clone(), current_committer.commit.clone()).begin.clone();

            ensure!(revealer_begin < current_begin, Error::<T>::NameAlreadyTaken);

            Committers::<T>::mutate(name.clone(), |x| *x = Committer {id: who.clone(), commit});
            Self::deposit_event(RawEvent::RegisterSucceeded(who, name, Some(current_committer.id)));
            Ok(())
        }

        #[weight = T::WeightInfo::renew()]
        /// Renew the "name" for "origin". The name should belong to "origin" in the first place.
        /// When successful, this will extend the register period by another "RegisterPeriod" since
        /// the renew time.
        pub fn renew(origin, name: T::Name) -> dispatch::DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(Committers::<T>::contains_key(name.clone()), Error::<T>::NameNotFound);

            let committer = Committers::<T>::get(name.clone());
            ensure!(committer.id == who.clone(), Error::<T>::NameNotOwned);

            ensure!(LockPeriods::<T>::contains_key(committer.id.clone(), committer.commit.clone()), Error::<T>::CommitNotFound);

            let end = <frame_system::Module<T>>::block_number() + T::RegisterPeriod::get();
            LockPeriods::<T>::mutate(committer.id, committer.commit, |value|{value.end = end;});

            Ok(())
        }

        #[weight = T::WeightInfo::unregister()]
        /// Unregister the name for origin and unlock the associated fund
        pub fn unregister(origin, name: T::Name) -> dispatch::DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(Committers::<T>::contains_key(name.clone()), Error::<T>::NameNotFound);

            let committer = Committers::<T>::get(name.clone());
            ensure!(committer.id == who.clone(), Error::<T>::NameNotOwned);

            ensure!(LockPeriods::<T>::contains_key(committer.id.clone(), committer.commit.clone()), Error::<T>::CommitNotFound);

            LockPeriods::<T>::remove(committer.id.clone(), committer.commit.clone());
            Committers::<T>::remove(name.clone());

            Self::update_locked_fund(who);

            Self::deposit_event(RawEvent::NameFreed(name));

            Ok(())
        }

        /// Find and remove expired commits and free the corresponding currency locks at block n.
        fn on_finalize(n: T::BlockNumber) {
            let expired_commits = Self::remove_expired_commits(n);
            expired_commits.iter().for_each(|committer| {
                Self::associated_name(committer.id.clone(), committer.commit.clone()).map(|name| {
                    Committers::<T>::remove(name.clone());
                    Self::deposit_event(RawEvent::NameFreed(name));
                });
            });
        }
    }
}

impl<T: Trait> Module<T> {
    /// Set lock according to the number of commits that are associated with and id.
    /// Remove the lock if no commits.
    fn update_locked_fund(id: T::AccountId) {
        let num_of_commits = LockPeriods::<T>::iter_prefix_values(id.clone()).count();
        if num_of_commits > 0 {
            // TODO check if checked ops is safer to be used below
            let amount_to_lock = T::FundToLock::get() * num_of_commits.saturated_into();
            T::Currency::set_lock(
                T::ModuleId::get(),
                &id,
                amount_to_lock,
                WithdrawReasons::all(),
            );
        } else {
            T::Currency::remove_lock(T::ModuleId::get(), &id);
        }
    }

    /// Remove expired commits for which the lock period is over.
    /// Return the list of expired commits together with the associated ids (Committer).
    fn remove_expired_commits(now: T::BlockNumber) -> Vec<Committer<T>> {
        let expired_commit_ids: Vec<Committer<T>> = LockPeriods::<T>::iter()
            .filter(|(_, _, lock_period)| lock_period.end <= now)
            .map(|(id, commit, _)| Committer { id, commit })
            .collect();
        expired_commit_ids.iter().for_each(|committer| {
            LockPeriods::<T>::remove(committer.id.clone(), committer.commit.clone());
            Self::update_locked_fund(committer.id.clone());
        });
        expired_commit_ids
    }

    /// Calculate the commit for "name" from "id" which the hash of 'id concatenated name'
    fn hash_of(id: T::AccountId, name: T::Name) -> T::Hash {
        let mut id_plus_name = id.encode();
        id_plus_name.extend_from_slice(&name.encode());
        T::Hashing::hash_of(&id_plus_name)
    }

    /// Find the name (if any) associated to a committer (id, commit) in the registry.
    /// Return None when no name is revealed for that commit yet.
    fn associated_name(id: T::AccountId, commit: T::Hash) -> Option<T::Name> {
        Committers::<T>::iter()
            .find(|(_, x)| {
                *x == Committer::<T> {
                    id: id.clone(),
                    commit: commit.clone(),
                }
            })
            .map(|(name, _)| name)
    }
}
