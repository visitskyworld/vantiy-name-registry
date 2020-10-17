use crate::{Module, Trait};
use frame_support::{impl_outer_origin, parameter_types, traits::LockIdentifier, weights::Weight};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
};

impl_outer_origin! {
    pub enum Origin for Test {}
}

// Configure a mock runtime to test the pallet.

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Test;
parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
}

impl system::Trait for Test {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Call = ();
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = ();
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type DbWeight = ();
    type BlockExecutionWeight = ();
    type ExtrinsicBaseWeight = ();
    type MaximumExtrinsicWeight = MaximumBlockWeight;
    type MaximumBlockLength = MaximumBlockLength;
    type AvailableBlockRatio = AvailableBlockRatio;
    type Version = ();
    type PalletInfo = ();
    type AccountData = pallet_balances::AccountData<u64>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
}

parameter_types! {
    pub const ExistentialDeposit: u64 = 0;
    pub const MaxLocks: u32 = 50;
}
impl pallet_balances::Trait for Test {
    type MaxLocks = MaxLocks;
    /// The type for recording an account's balance.
    type Balance = u64;
    /// The ubiquitous event type.
    type Event = ();
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
}

parameter_types! {
        pub const VanityRegistryId: LockIdentifier = *b"template";
        pub const RegisterPeriod: <Test as system::Trait>::BlockNumber = 95;
        pub const FundToLock: <Test as pallet_balances::Trait>::Balance = 57;
}
impl Trait for Test {
    type Event = ();
    type Currency = Balances;
    type ModuleId = VanityRegistryId;
    type RegisterPeriod = RegisterPeriod;
    type FundToLock = FundToLock;
    type Name = Vec<u8>;
    type WeightInfo = ();
}

pub type Template = Module<Test>;
pub type System = frame_system::Module<Test>;
pub type Balances = pallet_balances::Module<Test>;

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap()
        .into()
}
