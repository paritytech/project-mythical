//! This Substrate pallet provides a robust toolset for managing secure deposits within the blockchain's economic ecosystem.
//! It allows users to deposit funds that are held in escrow and managed through designated agents,
//! facilitating controlled and reversible transactions. Ideal for applications requiring high assurance
//! of fund preservation and manipulation under specific conditions.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

mod benchmarking;

mod mock;
mod tests;

pub mod weights;
pub use weights::*;

use frame_support::{
	pallet_prelude::*,
	traits::{
		fungible::{Inspect, Mutate, MutateHold},
		tokens::{Balance, Precision::Exact, Preservation::Expendable},
	},
};
use frame_system::pallet_prelude::*;
use sp_runtime::traits::{EnsureAddAssign, EnsureSubAssign};
use sp_std::prelude::*;

pub type BalanceOf<T> =
	<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo, Copy, MaxEncodedLen)]
pub struct Deposit<AccountId, Balance> {
	value: Balance,
	agent: AccountId,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_balances::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Currency: Inspect<Self::AccountId, Balance = <Self as Config>::Balance>
			+ Mutate<Self::AccountId>
			+ MutateHold<Self::AccountId, Reason = <Self as pallet::Config>::RuntimeHoldReason>;

		type Balance: Balance;

		type RuntimeHoldReason: From<HoldReason>;

		/// The minimum deposit value allowed.
		#[pallet::constant]
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
		/// A deposit was made.
		Deposited { account: T::AccountId, value: BalanceOf<T>, agent: T::AccountId },
		/// Funds were released from a deposit.
		Released { account: T::AccountId, value: BalanceOf<T>, agent: T::AccountId },
		/// A deposit was revoked and all deposited funds were transfered to the destination wallet.
		Revoked {
			account: T::AccountId,
			destination: T::AccountId,
			agent: T::AccountId,
			value: BalanceOf<T>,
			reason: Vec<u8>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// When trying to release more funds than are on deposit.
		InsufficientBalance,
		/// When the deposit value is less then configured value.
		DepositTooLow,
		/// When trying to release or revoke a deposit that does not exist.
		NoSuchDeposit,
		/// When the account balance is below the existential deposit before depositing.
		BalanceTooLow,
	}

	#[pallet::storage]
	pub type Deposits<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::AccountId,
		BalanceOf<T>,
		ValueQuery,
	>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Deposits a specified amount of funds directly into the balance of a target account by transferring from the balance of the origin.
		/// The deposited amount is reserved in the target account. The reserved funds can later be released partially or in full
		/// by an authorized escrow agent. This function ensures that the deposit meets or exceeds the minimum required balance.
		///
		/// # Arguments
		/// * `origin` - The origin of the transaction, whose balance the funds are transferred from.
		/// * `address` - The target account that will receive and hold the reserved funds.
		/// * `value` - The amount to be deposited and reserved.
		/// * `authorised_agent` - The agent authorized to manage and release the reserved funds.
		///
		/// # Errors
		/// * `Error::<T>::DepositTooLow` if the deposit amount is below the minimum threshold.
		/// * `Error::<T>::BalanceTooLow` if the target account balance is below the existential deposit.
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

		/// Releases a specified amount from the reserved balance of an account to its available balance.
		/// This action can only be initiated by an authorized escrow agent and ensures that the release does not exceed
		/// the reserved amount. This method is used primarily to reduce or clear the reservations made previously by the deposit action.
		///
		/// # Arguments
		/// * `origin` - The origin of the transaction, should be an authorized escrow agent.
		/// * `address` - The account holder of the deposited funds.
		/// * `value` - The amount to be released from the reserved balance.
		///
		/// # Errors
		/// * `Error::<T>::InsufficientBalance` if the reserved balance in the account is less than the amount requested to be released.
		/// * `Error::<T>::NoSuchDeposit` if there is no deposit record for the given accounts, indicating that no such reserved amount exists.
		#[pallet::weight(<T as Config>::WeightInfo::release())]
		#[pallet::call_index(1)]
		pub fn release(
			origin: OriginFor<T>,
			address: T::AccountId,
			value: BalanceOf<T>,
		) -> DispatchResult {
			let agent = ensure_signed(origin)?;

			Self::do_release(&address, &agent, value)
		}

		/// Revokes a reserved deposit, transferring the remaining reserved funds to a specified destination account for a specified reason.
		/// This function is typically used when a transaction or agreement fails to complete as planned, and the funds need to be returned or redirected.
		/// Only authorized agents can initiate a revocation to ensure control and security over the process.
		///
		/// # Arguments
		/// * `origin` - The origin of the transaction, should be an authorized escrow agent.
		/// * `address` - The account from which the reserved funds will be withdrawn.
		/// * `destination` - The account to which the funds will be transferred.
		/// * `reason` - A byte vector detailing the reason for the revocation, providing transparency and traceability.
		///
		/// # Errors
		/// * `Error::<T>::NoSuchDeposit` if there is no record of the reserved deposit for the given account and agent combination,
		///    indicating that no funds are available to be revoked.
		#[pallet::weight(<T as Config>::WeightInfo::revoke())]
		#[pallet::call_index(2)]
		pub fn revoke(
			origin: OriginFor<T>,
			address: T::AccountId,
			destination: T::AccountId,
			reason: Vec<u8>,
		) -> DispatchResult {
			let revoker = ensure_signed(origin)?;

			Deposits::<T>::mutate_exists(&address, &revoker, |maybe_deposit| -> DispatchResult {
				if let Some(deposit) = maybe_deposit.as_mut() {
					T::Currency::release(&HoldReason::Escrow.into(), &address, *deposit, Exact)?;

					T::Currency::transfer(&address, &destination, *deposit, Expendable)?;

					Self::deposit_event(Event::Revoked {
						account: address.clone(),
						destination: destination.clone(),
						agent: revoker.clone(),
						value: *deposit,
						reason,
					});

					*maybe_deposit = None;

					Ok(())
				} else {
					Err(Error::<T>::NoSuchDeposit.into())
				}
			})
		}

		/// Forcefully revokes a deposit under special conditions, overriding typical checks.
		/// This function is intended for emergency or administrative use where standard revocation processes are insufficient or inappropriate.
		/// It requires root privileges, underscoring its use in exceptional circumstances only.
		///
		/// # Arguments
		/// * `origin` - The origin of the transaction, which must be a root call to ensure administrative authority.
		/// * `address` - The account from which reserved funds will be moved.
		/// * `agent` - The agent initially authorized to manage the deposit, involved for traceability and records.
		/// * `destination` - The account to which the funds will be transferred, potentially different from the original depositor.
		/// * `reason` - A byte vector detailing the reason for the forced revocation, providing necessary context for this exceptional action.
		///
		/// # Errors
		/// * `Error::<T>::NoSuchDeposit` if there is no record of the reserved deposit for the given account, indicating that no funds are available to be forcibly revoked.
		#[pallet::weight(<T as Config>::WeightInfo::force_release())]
		#[pallet::call_index(3)]
		pub fn force_release(
			origin: OriginFor<T>,
			address: T::AccountId,
			agent: T::AccountId,
			value: BalanceOf<T>,
		) -> DispatchResult {
			ensure_root(origin)?;

			Self::do_release(&address, &agent, value)
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn integrity_test() {
			// Ensure that the minimum deposit is higher than the existential deposit.
			assert!(
				T::MinDeposit::get() >= T::Currency::minimum_balance(),
				"MinDeposit must be greater or equal to existential deposit."
			);
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
		ensure!(value >= Self::min_deposit(), Error::<T>::DepositTooLow);
		ensure!(
			T::Currency::balance(address) >= T::Currency::minimum_balance(),
			Error::<T>::BalanceTooLow
		);

		Deposits::<T>::try_mutate(address, authorised_agent, |deposit| -> DispatchResult {
			T::Currency::transfer(depositor, address, value, Expendable)?;
			T::Currency::hold(&HoldReason::Escrow.into(), address, value)?;

			Self::deposit_event(Event::Deposited {
				account: address.clone(),
				value,
				agent: authorised_agent.clone(),
			});

			deposit.ensure_add_assign(value)?;

			Ok(())
		})
	}

	pub fn do_release(
		address: &T::AccountId,
		agent: &T::AccountId,
		value: BalanceOf<T>,
	) -> DispatchResult {
		Deposits::<T>::mutate_exists(address, agent, |maybe_deposit| -> DispatchResult {
			if let Some(deposit) = maybe_deposit.as_mut() {
				deposit.ensure_sub_assign(value).map_err(|_| Error::<T>::InsufficientBalance)?;

				T::Currency::release(&HoldReason::Escrow.into(), address, value, Exact)?;

				Self::deposit_event(Event::Released {
					account: address.clone(),
					value,
					agent: agent.clone(),
				});

				Ok(())
			} else {
				Err(Error::<T>::NoSuchDeposit.into())
			}
		})
	}

	pub fn get_deposit(address: &T::AccountId, agent: &T::AccountId) -> BalanceOf<T> {
		Deposits::<T>::get(address, agent)
	}
}
