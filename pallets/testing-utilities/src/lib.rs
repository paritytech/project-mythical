//! Utility calls for external tools.
//!
//! This pallet is intended for testing behaviours present on mainnet that are
//! otherwise hard to achieve on testnets.
//!
//! Not intended to be used on mainnet.
#![cfg_attr(not(feature = "std"), no_std)]

mod benchmarking;
mod mock;
mod tests;

pub mod weights;
pub use weights::*;

pub use pallet::*;

use frame_support::{
	pallet_prelude::*,
	traits::{
		fungible::{Inspect, Mutate},
		tokens::{Fortitude, Precision, Preservation},
	},
	weights::WeightMeter,
};
use frame_system::pallet_prelude::{BlockNumberFor as SystemBlockNumberFor, *};
use sp_runtime::traits::{BlockNumberProvider, Saturating};
use sp_std::prelude::*;

/// The type that represents account balances in the runtime.
pub type BalanceOf<T> =
	<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

/// A transfer that was scheduled to be executed in
/// the next on_idle hook invocation.
#[derive(Clone, RuntimeDebug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo)]
pub struct ScheduledTransfer<AccountId, Balance> {
	from: AccountId,
	to: AccountId,
	amount: Balance,
}

/// A [ScheduledTransfer] parameterised with the runtime's config.
pub type ScheduledTransferOf<T> =
	ScheduledTransfer<<T as frame_system::Config>::AccountId, BalanceOf<T>>;

/// Runtime block number type.
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

		/// Runtime currency type to be used by the `transfer_through_delayed_remint` call.
		type Currency: Inspect<Self::AccountId>
			+ Mutate<Self::AccountId>;

		/// Runtime block number type.
		type BlockNumberProvider: BlockNumberProvider;

		/// Generated weights.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A delayed transfer was scheduled.
		TransferScheduled{ transfer: ScheduledTransferOf<T> },
		/// A delayed transfer was executed by the on_idle hook.
		TransferExecuted { scheduled_in: BlockNumberFor<T>, transfer: ScheduledTransferOf<T> },
		/// A delayed transfer has failed to execute.
		TransferFailed { scheduled_in: BlockNumberFor<T>, transfer: ScheduledTransferOf<T>, error: DispatchError },
	}

	/// A map from block number to a transfer that may have been scheduled in that block.
	#[pallet::storage]
	pub type ScheduledTransfers<T: Config> =
		StorageMap<_, Twox64Concat, BlockNumberFor<T>, ScheduledTransferOf<T>, OptionQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// Source balance is too low to transfer the required amount.
		SourceBalanceTooLow,
		/// Destination balance is too low even after transfer.
		DestinationBalanceTooLow,
		/// Transfer was already scheduled in this block.
		CouldNotSchedule,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Scheduled a transfer to be performed in the `on_idle` hook by means
		/// of burning the amount on the source account and minting it on the
		/// destination account.
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
		/// * `Error::<T>::CouldNotSchedule` when another task was already
		///   scheduled in this block.
		#[pallet::weight(<T as Config>::WeightInfo::transfer_through_delayed_remint())]
		#[pallet::call_index(0)]
		pub fn transfer_through_delayed_remint(
			origin: OriginFor<T>,
			to: T::AccountId,
			#[pallet::compact] amount: BalanceOf<T>,
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

			let tf = ScheduledTransfer { from, to, amount };
			<ScheduledTransfers<T>>::insert(current_block, tf.clone());

			Self::deposit_event(Event::TransferScheduled{ transfer: tf });

			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<SystemBlockNumberFor<T>> for Pallet<T> {
		fn on_idle(_n: SystemBlockNumberFor<T>, remaining_weight: Weight) -> Weight {
			let mut meter = WeightMeter::with_limit(remaining_weight);
			loop {
				if meter.try_consume(<T as Config>::WeightInfo::execute_scheduled_transfer()).is_ok() {
					let executed = Self::execute_scheduled_transfer();
					if !executed {
						 break;
					}
				} else {
					break;
				}
			}
			meter.remaining()
		}
	}

	impl<T: Config> Pallet<T> {
		/// Consumes and executes a single scheduled transfer. Returns true if
		/// a transfer was executed.
		pub(crate) fn execute_scheduled_transfer() -> bool {
			if let Some((block_number, tf)) = <ScheduledTransfers<T>>::iter().next() {
				<ScheduledTransfers<T>>::remove(block_number);

				if let Err(e) = T::Currency::burn_from(
					&tf.from,
					tf.amount,
					Preservation::Preserve,
					Precision::Exact,
					Fortitude::Polite,
				) {
					Self::deposit_event(Event::TransferFailed{ scheduled_in: block_number, transfer: tf, error: e });
					return true;
				}

				if let Err(e) = T::Currency::mint_into(&tf.to, tf.amount) {
					Self::deposit_event(Event::TransferFailed{ scheduled_in: block_number, transfer: tf, error: e });
					return true;
				}

				Self::deposit_event(Event::TransferExecuted { scheduled_in: block_number, transfer: tf });

				true
			} else {
				false
			}
		}
	}
}
