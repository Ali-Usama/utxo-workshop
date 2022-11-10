use pallet_aura::*;
use codec::{Decode, Encode};
use frame_support::{
    decl_event, decl_module, decl_storage,
    dispatch::{DispatchResult, Vec},
    ensure,
    weights::{
        constants::{RocksDbWeight}
    },
};
use sp_core::{H256, H512};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::sr25519::{Public, Signature};
use sp_runtime::traits::{BlakeTwo256, Hash, SaturatedConversion};
use sp_std::collections::btree_map::BTreeMap;
use sp_runtime::transaction_validity::{TransactionLongevity, ValidTransaction};

pub trait Config: frame_system::Config {
    type Event: From<Event> + Into<<Self as frame_system::Config>::Event>;
}

decl_storage! {
	trait Store for Module<T: Config> as Utxo {

	}
}

// External functions: callable by the end user
decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

	}
}

decl_event! {
	pub enum Event {

	}
}

/// Tests for this module
#[cfg(test)]
mod tests {
    use super::*;

    use frame_support::{assert_ok, assert_err, impl_outer_origin, parameter_types, weights::Weight};
    use sp_runtime::{testing::Header, traits::IdentityLookup, Perbill};
    use sp_core::testing::{KeyStore, SR25519};
    use sp_core::traits::KeystoreExt;
    use crate::Balance;
    // use crate::{Balance, Version};

    impl_outer_origin! {
		pub enum Origin for Test {}
	}

    #[derive(Clone, Eq, PartialEq)]
    pub struct Test;
    parameter_types! {
        pub const BlockHashCount: u64 = 250;
        pub const MaximumBlockWeight: Weight = 1024;
        pub const MaximumBlockLength: u32 = 2 * 1024;
        pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
        pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights
            ::with_sensible_defaults(2 * WEIGHT_PER_SECOND, NORMAL_DISPATCH_RATIO);
        pub BlockLength: frame_system::limits::BlockLength = frame_system::limits::BlockLength
            ::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
        pub const SS58Prefix: u8 = 42;
	}
    impl frame_system::Config for Test {
        type BaseCallFilter = frame_support::traits::Everything;
        type BlockWeights = BlockWeights;
        type BlockLength = BlockLength;
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
        type DbWeight = RocksDbWeight;
        type Version = ();
        type PalletInfo = PalletInfo;
        type AccountData = pallet_balances::AccountData<Balance>;
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type SystemWeightInfo = ();
        type SS58Prefix = SS58Prefix;
        type OnSetCode = ();
        type MaxConsumers = frame_support::traits::ConstU128<16>;
    }

    impl Config for Test {
        type Event = Event;
    }

    type Utxo = Module<Test>;
}
