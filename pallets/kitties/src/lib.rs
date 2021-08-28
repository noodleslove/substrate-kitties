#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	pallet_prelude::*, Parameter,
	traits::{Randomness, Currency, ExistenceRequirement},
};
use frame_system::pallet_prelude::*;
use sp_runtime::{
    ArithmeticError,
    traits::{AtLeast32BitUnsigned, Bounded, One, CheckedAdd}
};
use sp_io::hashing::blake2_128;
use sp_std::result::Result;

pub use pallet::*;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq)]
    pub struct Kitty(pub [u8; 16]);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_balances::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Randomness: Randomness<Self::Hash, Self::BlockNumber>;
		type KittyIndex: Parameter + AtLeast32BitUnsigned + Bounded + Default + Copy;
	}

    #[pallet::storage]
	#[pallet::getter(fn kitties)]
	pub type Kitties<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat, T::AccountId,
		Blake2_128Concat, T::KittyIndex,
		Kitty, OptionQuery
	>;

    #[pallet::storage]
	#[pallet::getter(fn next_kitty_id)]
	pub type NextKittyId<T: Config> = StorageValue<_, T::KittyIndex, ValueQuery>;

    #[pallet::storage]
	#[pallet::getter(fn kitty_prices)]
	pub type KittyPrices<T: Config> = StorageMap<
		_,
		Blake2_128Concat, T::KittyIndex,
		T::Balance, OptionQuery
	>;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	#[pallet::metadata(
		T::AccountId = "AccountId", T::KittyIndex = "KittyIndex", Option<T::Balance> = "Option<Balance>", T::Balance = "Balance",
	)]
	pub enum Event<T: Config> {
		/// A kitty is created. \[owner, kitty_id, kitty\]
		KittyCreated(T::AccountId, T::KittyIndex, Kitty),
		/// A new kitten is bred. \[owner, kitty_id, kitty\]
		KittyBred(T::AccountId, T::KittyIndex, Kitty),
		/// A kitty is transferred. \[from, to, kitty_id\]
		KittyTransferred(T::AccountId, T::AccountId, T::KittyIndex),
		/// The price for a kitty is updated. \[owner, kitty_id, price\]
		KittyPriceUpdated(T::AccountId, T::KittyIndex, Option<T::Balance>),
		/// A kitty is sold. \[old_owner, new_owner, kitty_id, price\]
		KittySold(T::AccountId, T::AccountId, T::KittyIndex, T::Balance),
	}

	#[pallet::error]
	pub enum Error<T> {
		InvalidKittyId,
		CannotSameParent,
		NotOwner,
		NotForSale,
		PriceTooLow,
		CannotBuyFromSelf,
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		
        /// Create a new kitty
		#[pallet::weight(1000)]
		pub fn create(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let kitty_id = Self::get_next_kitty_id()?;

			let dna = Self::random_value(&who);

			// Create and store kitty
			let kitty = Kitty(dna);
			Kitties::<T>::insert(&who, kitty_id, &kitty);

			// Emit event
			Self::deposit_event(Event::KittyCreated(who, kitty_id, kitty));

			Ok(())
		}


		/// Breed kitties
		#[pallet::weight(1000)]
		pub fn breed(
			origin: OriginFor<T>,
			kitty_id_1: T::KittyIndex,
			kitty_id_2: T::KittyIndex
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let kitty1 = Self::kitties(&who, kitty_id_1).ok_or(Error::<T>::InvalidKittyId)?;
			let kitty2 = Self::kitties(&who, kitty_id_2).ok_or(Error::<T>::InvalidKittyId)?;

			ensure!(kitty1 != kitty2, Error::<T>::CannotSameParent);

			let kitty_id = Self::get_next_kitty_id()?;

			let kitty1_dna = kitty1.0;
			let kitty2_dna = kitty2.0;

			let selector = Self::random_value(&who);
			let mut new_dna = [0u8; 16];

			// Combine parents and selector to create new kitty
			for i in 0..kitty1_dna.len() {
				new_dna[i] = combine_dna(kitty1_dna[i], kitty2_dna[i], selector[i]);
			}

			let new_kitty = Kitty(new_dna);

			Kitties::<T>::insert(&who, kitty_id, &new_kitty);

			Self::deposit_event(Event::KittyBred(who, kitty_id, new_kitty));

			Ok(())
		}

		/// Transfer a kitty to new owner
		#[pallet::weight(1000)]
		pub fn transfer(origin: OriginFor<T>, to: T::AccountId, kitty_id: T::KittyIndex) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			Kitties::<T>::try_mutate_exists(sender.clone(), kitty_id, |kitty| -> DispatchResult {
				if sender == to {
					ensure!(kitty.is_some(), Error::<T>::InvalidKittyId);
					return Ok(());
				}

				let kitty = kitty.take().ok_or(Error::<T>::InvalidKittyId)?;

				Kitties::<T>::insert(&to, kitty_id, kitty);

				KittyPrices::<T>::remove(kitty_id);

				Self::deposit_event(Event::KittyTransferred(sender, to, kitty_id));

				Ok(())
			})
		}

		/// Set a price for a kitty for sale
 		/// None to delist the kitty
		#[pallet::weight(1000)]
		pub fn set_price(
			origin: OriginFor<T>,
			kitty_id: T::KittyIndex,
			new_price: Option<T::Balance>
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(<Kitties<T>>::contains_key(&who, kitty_id), Error::<T>::NotOwner);

			KittyPrices::<T>::mutate_exists(kitty_id, |price| *price = new_price);

			Self::deposit_event(Event::KittyPriceUpdated(who, kitty_id, new_price));

			Ok(())
		}

		/// Buy a kitty
		#[pallet::weight(1000)]
		pub fn buy(
			origin: OriginFor<T>,
			owner: T::AccountId,
			kitty_id: T::KittyIndex,
			max_price: T::Balance
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(who != owner, Error::<T>::CannotBuyFromSelf);

			Kitties::<T>::try_mutate_exists(owner.clone(), kitty_id, |kitty| -> DispatchResult {
				let kitty = kitty.take().ok_or(Error::<T>::InvalidKittyId)?;

				KittyPrices::<T>::try_mutate_exists(kitty_id, |price| -> DispatchResult {
					let price = price.take().ok_or(Error::<T>::NotForSale)?;

					ensure!(max_price >= price, Error::<T>::PriceTooLow);

					<pallet_balances::Pallet<T> as Currency<T::AccountId>>::transfer(&who, &owner, price, ExistenceRequirement::KeepAlive)?;

					Kitties::<T>::insert(&who, kitty_id, kitty);

					Self::deposit_event(Event::KittySold(owner, who, kitty_id, price));

					Ok(())
				})
			})
		}
	}

    fn combine_dna(dna1: u8, dna2: u8, selector: u8) -> u8 {
        (!selector & dna1) | (selector & dna2)
    }
    
    impl<T: Config> Pallet<T> {
        fn get_next_kitty_id() -> Result<T::KittyIndex, DispatchError> {
            NextKittyId::<T>::try_mutate(|next_id| -> Result<T::KittyIndex, DispatchError> {
                let current_id = *next_id;
                *next_id = next_id.checked_add(&One::one()).ok_or(ArithmeticError::Overflow)?;
                Ok(current_id)
            })
        }
    
        fn random_value(sender: &T::AccountId) -> [u8; 16] {
            let payload = (
                T::Randomness::random_seed().0,
                &sender,
                <frame_system::Pallet<T>>::extrinsic_index(),
            );
            payload.using_encoded(blake2_128)
        }
    }
}
