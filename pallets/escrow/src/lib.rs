#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

mod benchmarking;

mod mock;
mod tests;

pub mod weights;

use frame_support::{
	pallet_prelude::*,
	traits::{
		fungible::{Inspect, Mutate, MutateHold},
		tokens::{Precision::Exact, Preservation::Preserve},
	},
};
use frame_system::pallet_prelude::*;
use sp_runtime::traits::Zero;
use sp_std::prelude::*;
use weights::WeightInfo;

// type BalanceOf<T> =
//     <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub type BalanceOf<T> =
	<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo, Copy, MaxEncodedLen)]
pub struct Deposit<AccountId, Balance> {
	value: Balance,
	agent: AccountId,
}

// #[frame_support::pallet(dev_mode)]
#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_balances::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Currency: Inspect<Self::AccountId>
			+ Mutate<Self::AccountId>
			+ MutateHold<Self::AccountId, Reason = <Self as pallet::Config>::RuntimeHoldReason>;

		type RuntimeHoldReason: From<HoldReason>;

		type MaxDeposits: Get<u32>;

		type MinDeposit: Get<BalanceOf<Self>>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::composite_enum]
	pub enum HoldReason {
		Escrow,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Deposited { account: T::AccountId, value: BalanceOf<T>, agent: T::AccountId },
		Released { account: T::AccountId, value: BalanceOf<T> },
		Revoked { account: T::AccountId, destination: T::AccountId, reason: Vec<u8> },
	}

	#[pallet::error]
	pub enum Error<T> {
		Unauthorized,
		TooManyDeposits,
		InsufficientBalance,
		DepositTooSmall,
	}

	#[pallet::storage]
	#[pallet::getter(fn deposits)]
	pub type Deposits<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		BoundedVec<Deposit<T::AccountId, BalanceOf<T>>, T::MaxDeposits>,
		ValueQuery,
	>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(<T as Config>::WeightInfo::deposit())]
		#[pallet::call_index(0)]
		pub fn deposit(
			origin: OriginFor<T>,
			address: T::AccountId,
			value: BalanceOf<T>,
			authorised_agent: T::AccountId,
		) -> DispatchResult {
			let depositor = ensure_signed(origin.clone())?;

			Self::make_deposit(&depositor, &address, value, &authorised_agent)
		}

		#[pallet::weight(<T as Config>::WeightInfo::release())]
		#[pallet::call_index(1)]
		pub fn release(
			origin: OriginFor<T>,
			address: T::AccountId,
			value: BalanceOf<T>,
		) -> DispatchResult {
			let releaser = ensure_signed(origin)?;

			Deposits::<T>::mutate_exists(&address, |maybe_deposits| -> DispatchResult {
				if let Some(deposits) = maybe_deposits.as_mut() {
					match deposits.binary_search_by_key(&releaser, |d| d.agent.clone()) {
						Ok(index) => {
							ensure!(
								deposits[index].value >= value,
								Error::<T>::InsufficientBalance
							);

							deposits[index].value -= value;

							T::Currency::release(
								&HoldReason::Escrow.into(),
								&address,
								value,
								Exact,
							)?;

							Self::deposit_event(Event::Released {
								account: address.clone(),
								value,
							});

							Ok(())
						},
						Err(_) => Err(Error::<T>::Unauthorized.into()),
					}
				} else {
					Err(Error::<T>::InsufficientBalance.into())
				}
			})
		}

		#[pallet::weight(<T as Config>::WeightInfo::revoke())]
		#[pallet::call_index(2)]
		pub fn revoke(
			origin: OriginFor<T>,
			address: T::AccountId,
			destination: T::AccountId,
			reason: Vec<u8>,
		) -> DispatchResult {
			let revoker = ensure_signed(origin)?;

			Deposits::<T>::mutate_exists(&address, |maybe_deposits| -> DispatchResult {
				if let Some(deposits) = maybe_deposits.as_mut() {
					match deposits.binary_search_by_key(&revoker, |d| d.agent.clone()) {
						Ok(index) => {
							let deposit = deposits.remove(index);

							T::Currency::release(
								&HoldReason::Escrow.into(),
								&address,
								deposit.value,
								Exact,
							)?;

							T::Currency::transfer(&address, &destination, deposit.value, Preserve)?;

							Self::deposit_event(Event::Revoked {
								account: address.clone(),
								destination: destination.clone(),
								reason,
							});

							Ok(())
						},
						Err(_) => Err(Error::<T>::Unauthorized.into()),
					}
				} else {
					Err(Error::<T>::InsufficientBalance.into())
				}
			})
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn min_deposit() -> BalanceOf<T> {
		T::MinDeposit::get()
	}

	pub fn make_deposit(
		depositor: &T::AccountId,
		address: &T::AccountId,
		value: BalanceOf<T>,
		authorised_agent: &T::AccountId,
	) -> DispatchResult {
		ensure!(value >= T::MinDeposit::get(), Error::<T>::DepositTooSmall);

		return Deposits::<T>::try_mutate(&address, |deposits| -> DispatchResult {
			T::Currency::transfer(&depositor, &address, value, Preserve)?;

			let resulting_balance = T::Currency::balance(&address);
			let min_balance = T::Currency::minimum_balance();

			let to_reserve =
				if resulting_balance - value < min_balance { value - min_balance } else { value };

			T::Currency::hold(&HoldReason::Escrow.into(), &address, to_reserve)?;

			Self::deposit_event(Event::Deposited {
				account: address.clone(),
				value,
				agent: authorised_agent.clone(),
			});

			match deposits.binary_search_by_key(authorised_agent, |d| d.agent.clone()) {
				Ok(index) => {
					deposits[index].value += to_reserve;
				},

				Err(index) => {
					deposits
						.try_insert(
							index,
							Deposit { value: to_reserve, agent: authorised_agent.clone() },
						)
						.map_err(|_| Error::<T>::TooManyDeposits)?;
				},
			};

			Ok(())
		});
	}

	pub fn total_deposited(account: &T::AccountId) -> BalanceOf<T> {
		Deposits::<T>::get(account)
			.into_iter()
			.fold(Zero::zero(), |acc, d| acc + d.value)
	}
}
