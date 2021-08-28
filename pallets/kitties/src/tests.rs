use super::*;

use crate as pallet_kitties;
use sp_core::H256;
use frame_support::{parameter_types, assert_ok, assert_noop};
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup}, testing::Header,
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		KittiesModule: pallet_kitties::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::AllowAll;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}
impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type Balance = u64;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

parameter_types! {
	pub static MockRandom: H256 = Default::default();
}

impl Randomness<H256, u64> for MockRandom {
    fn random(_subject: &[u8]) -> (H256, u64) {
        (MockRandom::get(), 0)
    }
}

impl Config for Test {
	type Event = Event;
	type Randomness = MockRandom;
	type KittyIndex = u32;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

	pallet_balances::GenesisConfig::<Test>{
		balances: vec![(200, 500)],
	}.assimilate_storage(&mut t).unwrap();

	let mut t: sp_io::TestExternalities = t.into();

	t.execute_with(|| System::set_block_number(1) );
	t
}

#[test]
fn create_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(KittiesModule::create(Origin::signed(100)));

		let kitty = Kitty([1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1]);

		assert_eq!(KittiesModule::kitties(100, 0), Some(kitty.clone()));
		assert_eq!(KittiesModule::next_kitty_id(), 1);

		System::assert_last_event(Event::KittiesModule(crate::Event::<Test>::KittyCreated(100, 0, kitty)));
	});
}

#[test]
fn breed_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(KittiesModule::create(Origin::signed(100)));

		MockRandom::set(H256::from([2; 32]));

		assert_ok!(KittiesModule::create(Origin::signed(100)));

		assert_noop!(KittiesModule::breed(Origin::signed(100), 0, 11), Error::<Test>::InvalidKittyId);
		assert_noop!(KittiesModule::breed(Origin::signed(100), 0, 0), Error::<Test>::CannotSameParent);
		assert_noop!(KittiesModule::breed(Origin::signed(101), 0, 1), Error::<Test>::InvalidKittyId);

		assert_ok!(KittiesModule::breed(Origin::signed(100), 0, 1));

		let kitty = Kitty([1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1]);

		assert_eq!(KittiesModule::kitties(100, 2), Some(kitty.clone()));
		assert_eq!(KittiesModule::next_kitty_id(), 3);

		System::assert_last_event(Event::KittiesModule(crate::Event::<Test>::KittyBred(100u64, 2u32, kitty)));
	});
}

#[test]
fn transfer_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(KittiesModule::create(Origin::signed(100)));
		assert_ok!(KittiesModule::set_price(Origin::signed(100), 0, Some(10)));

		assert_noop!(KittiesModule::transfer(Origin::signed(101), 200, 0), Error::<Test>::InvalidKittyId);

		assert_ok!(KittiesModule::transfer(Origin::signed(100), 200, 0));

		let kitty = Kitty([1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1]);

		assert_eq!(KittiesModule::kitties(200, 0), Some(kitty));
		assert_eq!(Kitties::<Test>::contains_key(100, 0), false);
		assert_eq!(KittyPrices::<Test>::contains_key(0), false);

		System::assert_last_event(Event::KittiesModule(crate::Event::KittyTransferred(100, 200, 0)));
	});
}

#[test]
fn self_transfer_should_fail() {
	new_test_ext().execute_with(|| {
		assert_ok!(KittiesModule::create(Origin::signed(100)));

		System::reset_events();

		assert_noop!(KittiesModule::transfer(Origin::signed(100), 100, 1), Error::<Test>::InvalidKittyId);

		assert_ok!(KittiesModule::transfer(Origin::signed(100), 100, 0));

		let kitty = Kitty([1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1]);

		assert_eq!(KittiesModule::kitties(100, 0), Some(kitty));

		// no transfer event because no actual transfer is executed
		assert_eq!(System::events().len(), 0);
	});
}

#[test]
fn set_price_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(KittiesModule::create(Origin::signed(100)));

		assert_noop!(KittiesModule::set_price(Origin::signed(200), 0, Some(10)), Error::<Test>::NotOwner);

		assert_ok!(KittiesModule::set_price(Origin::signed(100), 0, Some(10)));

		System::assert_last_event(Event::KittiesModule(crate::Event::KittyPriceUpdated(100, 0, Some(10))));

		assert_eq!(KittiesModule::kitty_prices(0), Some(10));

		assert_ok!(KittiesModule::set_price(Origin::signed(100), 0, None));
		assert_eq!(KittyPrices::<Test>::contains_key(0), false);

		System::assert_last_event(Event::KittiesModule(crate::Event::KittyPriceUpdated(100, 0, None)));
	});
}

#[test]
fn buy_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(KittiesModule::create(Origin::signed(100)));

		let kitty = KittiesModule::kitties(100, 0).unwrap();

		assert_noop!(KittiesModule::buy(Origin::signed(100), 100, 0, 10), Error::<Test>::CannotBuyFromSelf);
		assert_noop!(KittiesModule::buy(Origin::signed(200), 100, 1, 10), Error::<Test>::InvalidKittyId);
		assert_noop!(KittiesModule::buy(Origin::signed(200), 100, 0, 10), Error::<Test>::NotForSale);

		assert_ok!(KittiesModule::set_price(Origin::signed(100), 0, Some(600)));

		assert_noop!(KittiesModule::buy(Origin::signed(200), 100, 0, 500), Error::<Test>::PriceTooLow);

		assert_noop!(KittiesModule::buy(Origin::signed(200), 100, 0, 600), pallet_balances::Error::<Test, _>::InsufficientBalance);

		assert_ok!(KittiesModule::set_price(Origin::signed(100), 0, Some(400)));

		assert_ok!(KittiesModule::buy(Origin::signed(200), 100, 0, 500));

		assert_eq!(KittyPrices::<Test>::contains_key(0), false);
		assert_eq!(Kitties::<Test>::contains_key(100, 0), false);
		assert_eq!(KittiesModule::kitties(200, 0), Some(kitty));
		assert_eq!(Balances::free_balance(100), 400);
		assert_eq!(Balances::free_balance(200), 100);

		System::assert_last_event(Event::KittiesModule(crate::Event::KittySold(100, 200, 0, 400)));
	});
}
