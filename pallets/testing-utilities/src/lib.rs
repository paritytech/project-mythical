//! Utilities for testing behaviours present on mainnet that are otherwise hard
//! to achieve in testnets.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::*;

pub use pallet::*;

use frame_support::{
	pallet_prelude::*,
	traits::{
		fungible::{Inspect, Mutate},
		tokens::{Balance, Fortitude, Precision, Preservation},
	},
};
use frame_system::pallet_prelude::{BlockNumberFor as SystemBlockNumberFor, *};
use sp_runtime::traits::{BlockNumberProvider, Saturating};
use sp_std::prelude::*;

pub type BalanceOf<T> =
	<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

#[derive(Clone, RuntimeDebug, PartialEq, Eq, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct ScheduledTransfer<AccountId, Balance> {
	from: AccountId,
	to: AccountId,
	amount: Balance,
}

pub type ScheduledTransferOf<T> =
	ScheduledTransfer<<T as frame_system::Config>::AccountId, <T as Config>::Balance>;

pub type BlockNumberFor<T> =
	<<T as Config>::BlockNumberProvider as BlockNumberProvider>::BlockNumber;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_balances::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Currency: Inspect<Self::AccountId, Balance = <Self as Config>::Balance>
			+ Mutate<Self::AccountId>;

		type Balance: Balance;

		type BlockNumberProvider: BlockNumberProvider;

		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Scheduled,
		Executed { scheduled_in: BlockNumberFor<T> },
	}

	#[pallet::storage]
	pub type ScheduledTransfers<T: Config> =
		StorageMap<_, Twox64Concat, BlockNumberFor<T>, ScheduledTransferOf<T>, OptionQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// Source balance is too low to transfer the required amount.
		SourceBalanceTooLow,
		/// Destination balance is too low even after transfer.
		DestinationBalanceTooLow,
		/// Unable to schedule due to agenda being full,
		CouldNotSchedule,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Schedules on the next block a `burn` of the funds on the `from` wallet
		/// with immediate `mint` on the `to` wallet.
		///
		/// # Arguments
		/// * `origin` - The origin of the transaction, source account for transfer.
		/// * `to` - Destination account for the balance transfer.
		/// * `amount` - The amount to be burned and reminted.
		///
		/// # Errors
		/// * `Error::<T>::SourceBalanceTooLow` when `from` balance is too low
		///   to transfer the required amount.
		/// * `Error::<T>::DestinationBalanceTooLow` when `to` balance will be
		///   below minimum after transfer.
		#[pallet::weight(<T as Config>::WeightInfo::transfer_through_delayed_remint())]
		#[pallet::call_index(0)]
		pub fn transfer_through_delayed_remint(
			origin: OriginFor<T>,
			to: T::AccountId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let from = ensure_signed(origin.clone())?;

			ensure!(
				T::Currency::balance(&from).saturating_sub(amount)
					>= T::Currency::minimum_balance(),
				<Error<T>>::SourceBalanceTooLow,
			);

			ensure!(
				T::Currency::balance(&to).saturating_add(amount) >= T::Currency::minimum_balance(),
				<Error<T>>::DestinationBalanceTooLow,
			);

			let current_block = T::BlockNumberProvider::current_block_number();

			ensure!(
				!<ScheduledTransfers<T>>::contains_key(&current_block),
				<Error<T>>::CouldNotSchedule,
			);

			<ScheduledTransfers<T>>::insert(current_block, ScheduledTransfer { from, to, amount });

			Self::deposit_event(Event::Scheduled);

			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<SystemBlockNumberFor<T>> for Pallet<T> {
		fn on_idle(_n: SystemBlockNumberFor<T>, _remaining_weight: Weight) -> Weight {
			if let Some((key, tf)) = <ScheduledTransfers<T>>::iter().next() {
				<ScheduledTransfers<T>>::remove(key);

				let _ = T::Currency::burn_from(
					&tf.from,
					tf.amount,
					Preservation::Preserve,
					Precision::Exact,
					Fortitude::Polite,
				);
				let _ = T::Currency::mint_into(&tf.to, tf.amount);
			}

			Weight::zero()
		}
	}
}
