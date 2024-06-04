//! # Proxy Module
//! A module that allows accounts to give permission to other accounts to make calls on their behalf.
//!
//! Main entities:
//! - **Delegator**: The account that gives permission to another account to make calls on its behalf.
//! - **Delegate**: The account that is given permission to make calls on behalf of the delegator.
//! - **Sponsor**: An account that can pay the deposit for the proxy. Sponsor has permission to remove
//! proxies that they have paid the deposit for. It should be secure cold wallet.
//! - **Sponsor Agent**: An account authorized by the Sponsor to initiate the funding of proxies using the Sponsor’s resources.
//! This role is designed to facilitate transactions while minimizing direct exposure of the Sponsor’s credentials.
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::*;

use frame_support::{
	dispatch::{extract_actual_weight, GetDispatchInfo, PostDispatchInfo},
	pallet_prelude::*,
	traits::{
		fungible::{Inspect, Mutate, MutateHold},
		tokens::Precision,
		InstanceFilter, IsSubType, OriginTrait,
	},
	weights::WeightMeter,
};
use frame_system::pallet_prelude::*;
use sp_runtime::traits::Dispatchable;
use sp_std::boxed::Box;

pub type BalanceOf<T> =
	<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

#[derive(
	Encode,
	Decode,
	Clone,
	Copy,
	Eq,
	PartialEq,
	Ord,
	PartialOrd,
	RuntimeDebug,
	MaxEncodedLen,
	TypeInfo,
)]
pub struct ProxyDefinition<AccountId, ProxyType> {
	/// A value defining the subset of calls that it is allowed to make.
	pub proxy_type: ProxyType,

	/// The account that is stacking the deposit for this proxy. If `None`, then it's the delegator.
	pub sponsor: Option<AccountId>,
}

#[frame_support::pallet]
pub mod pallet {

	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type RuntimeCall: Parameter
			+ Dispatchable<RuntimeOrigin = Self::RuntimeOrigin, PostInfo = PostDispatchInfo>
			+ GetDispatchInfo
			+ From<frame_system::Call<Self>>
			+ IsSubType<Call<Self>>
			+ IsType<<Self as frame_system::Config>::RuntimeCall>;

		type ProxyType: Parameter
			+ Member
			+ Ord
			+ PartialOrd
			+ InstanceFilter<<Self as Config>::RuntimeCall>
			+ Default
			+ MaxEncodedLen;

		type Currency: Inspect<Self::AccountId>
			+ Mutate<Self::AccountId>
			+ MutateHold<Self::AccountId, Reason = <Self as pallet::Config>::RuntimeHoldReason>;

		type RuntimeHoldReason: From<HoldReason>;

		type MaxProxies: Get<u32>;

		type ProxyDeposit: Get<BalanceOf<Self>>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::composite_enum]
	pub enum HoldReason {
		ProxyDeposit,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new proxy permission was added.
		ProxyCreated {
			delegator: T::AccountId,
			delegate: T::AccountId,
			proxy_type: T::ProxyType,
			sponsor: Option<T::AccountId>,
		},

		/// A proxy permission was removed.
		ProxyRemoved {
			delegator: T::AccountId,
			delegate: T::AccountId,
			/// The account that removed the proxy. If `None`, then it was the delegator.
			removed_by_sponsor: Option<T::AccountId>,
		},

		/// Proxy funding was approved.
		ProxySponsorshipApproved {
			delegator: T::AccountId,
			sponsor: T::AccountId,
			approver: T::AccountId,
		},

		/// A sponsor agent was registered.
		SponsorAgentRegistered { sponsor: T::AccountId, agent: T::AccountId },

		/// A sponsor agent was revoked.
		SponsorAgentRevoked { sponsor: T::AccountId, agent: T::AccountId },

		/// Proxy call was executed.
		/// This event is emitted only when the proxy call is successful.
		ProxyExecuted { delegator: T::AccountId, delegate: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The caller does not have the necessary permissions.
		Unauthorized,
		/// The specified proxy does not exist.
		NoSuchProxy,
		/// The delegator is not authorized to use the sponsor's funds.
		SponsorshipUnauthorized,
		/// The sponsor agent is not authorized to use the sponsor's funds.
		SponsorAgentUnauthorized,
		/// The delegate doesn't have proxy permission from the delegator.
		NotProxy,
		/// The sponsor agent is already registered.
		SponsorAgentAlreadyRegistered,
	}

	#[pallet::storage]
	pub type Proxies<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId, // delegator
		Blake2_128Concat,
		T::AccountId, // delegate
		ProxyDefinition<T::AccountId, T::ProxyType>,
		OptionQuery,
	>;

	/// A mapping from a sponsor agent to the sponsor.
	#[pallet::storage]
	pub type SponsorAgents<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId, OptionQuery>;

	/// A mapping from a delegator and a sponsor to the sponsor agent.
	#[pallet::storage]
	pub type SponsorshipApprovals<T: Config> =
		StorageMap<_, Blake2_128Concat, (T::AccountId, T::AccountId), T::AccountId, OptionQuery>;

	/// A mapping from a sponsor agent to the approval.
	/// This is used to clean up approvals after removing the agent.
	#[pallet::storage]
	pub type ApprovalsByAgent<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		(T::AccountId, T::AccountId),
		(),
		ValueQuery,
	>;

	/// Storage of agents that have been invalidated.
	/// This is used to clean up approvals that are no longer valid.
	#[pallet::storage]
	pub type InvalidatedAgents<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, (), ValueQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Adds a new proxy.
		///
		/// This extrinsic allows a delegator to grant permission to a delegate account to act on their behalf
		/// for a specific subset of calls defined by `proxy_type`. Optionally, a sponsor can be specified who will
		/// reserve the deposit required for the proxy. The reserved deposit is returned when the proxy is removed.
		///
		/// Emits `ProxyCreated` event.
		///
		/// # Parameters
		/// - `origin`: The delegator's account.
		/// - `delegate`: The account that is granted the proxy permission.
		/// - `proxy_type`: The type of proxy, which defines the subset of calls that the delegate can make on behalf of the delegator.
		/// - `sponsor`: (Optional) The account that will reserve the deposit for the proxy. If not provided, the delegator's balance will be reserved.
		///
		/// # Errors
		/// - `SponsorshipUnauthorized`: If the sponsor did not approve the delegator to use their funds.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::add_proxy())]
		pub fn add_proxy(
			origin: OriginFor<T>,
			delegate: T::AccountId,
			proxy_type: T::ProxyType,
			sponsor: Option<T::AccountId>,
		) -> DispatchResult {
			let delegator = ensure_signed(origin.clone())?;

			let proxy =
				ProxyDefinition { proxy_type: proxy_type.clone(), sponsor: sponsor.clone() };

			match sponsor.clone() {
				Some(sponsor) => {
					ensure!(
						Self::has_sponsorship_approval(&delegator.clone(), &sponsor),
						Error::<T>::SponsorshipUnauthorized
					);
					Self::remove_approval(&delegator, &sponsor);

					T::Currency::hold(
						&HoldReason::ProxyDeposit.into(),
						&sponsor,
						T::ProxyDeposit::get(),
					)?;
				},
				None => {
					T::Currency::hold(
						&HoldReason::ProxyDeposit.into(),
						&delegator,
						T::ProxyDeposit::get(),
					)?;
				},
			}

			Proxies::<T>::insert(&delegator, &delegate, proxy);

			Self::deposit_event(Event::ProxyCreated { delegator, delegate, proxy_type, sponsor });

			Ok(())
		}

		/// Executes a call on behalf of the delegator.
		///
		/// This extrinsic allows a delegate account to execute a call on behalf of the delegator,
		/// provided the delegate has the appropriate proxy permission. The call must be within the
		/// subset of allowed calls defined by the proxy type.
		///
		/// Emits `ProxyExecuted` event on success. If the call fails, the error is returned.
		///
		/// # Parameters
		/// - `origin`: The delegate's account.
		/// - `address`: The delegator's account on whose behalf the call is made.
		/// - `call`: The call to be executed.
		///
		/// # Errors
		/// - `NotProxy`: If the delegate does not have proxy permission from the delegator.
		/// - `frame_system::Error::<T>::CallFiltered`: If the call is not within the allowed subset of calls for the proxy.
		#[pallet::call_index(1)]
		#[pallet::weight({
			let di = call.get_dispatch_info();
			T::WeightInfo::proxy().saturating_add(di.weight)
		})]
		pub fn proxy(
			origin: OriginFor<T>,
			address: T::AccountId,
			call: Box<<T as Config>::RuntimeCall>,
		) -> DispatchResultWithPostInfo {
			let delegate = ensure_signed(origin)?;

			let maybe_proxy = Proxies::<T>::get(&address, &delegate);
			let proxy_def = maybe_proxy.ok_or(Error::<T>::NotProxy)?;

			let mut new_origin: T::RuntimeOrigin =
				frame_system::RawOrigin::Signed(address.clone()).into();

			let delegator = address.clone();

			new_origin.add_filter(move |c: &<T as frame_system::Config>::RuntimeCall| {
				let c = <T as Config>::RuntimeCall::from_ref(c);

				match c.is_sub_type() {
					// Proxy call cannot add a proxy with more permissions than it already has.
					Some(Call::add_proxy { ref proxy_type, .. })
						if !proxy_def.proxy_type.is_superset(proxy_type) =>
					{
						false
					},

					Some(Call::remove_proxy { ref delegate }) => {
						let removing_proxy_def = Proxies::<T>::get(&delegator, delegate);

						match removing_proxy_def {
							Some(removing_proxy_def) => {
								// Proxy call cannot remove a proxy with more permissions than it already has.
								proxy_def.proxy_type.is_superset(&removing_proxy_def.proxy_type)
							},
							None => true,
						}
					},
					_ => proxy_def.proxy_type.filter(c),
				}
			});

			let result = call.clone().dispatch(new_origin);

			Self::deposit_event(Event::ProxyExecuted { delegator: address, delegate });
			let base_weight = <T as Config>::WeightInfo::proxy();
			let call_weight = extract_actual_weight(&result, &call.get_dispatch_info());

			let weight = base_weight.saturating_add(call_weight);

			result.map_err(|mut err| {
				err.post_info = Some(weight).into();
				err
			})?;

			Ok(Some(weight).into())
		}

		/// Removes an existing proxy.
		///
		/// This extrinsic allows a delegator to remove a proxy permission previously granted to a delegate.
		/// If a sponsor was specified during the proxy creation, the reserved deposit is returned to the sponsor.
		///
		/// Emits `ProxyRemoved` event.
		///
		/// # Parameters
		/// - `origin`: The delegator's account.
		/// - `delegate`: The account whose proxy permission is to be removed.
		///
		/// # Errors
		/// - `NoSuchProxy`: If the proxy does not exist.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::remove_proxy())]
		pub fn remove_proxy(origin: OriginFor<T>, delegate: T::AccountId) -> DispatchResult {
			let delegator = ensure_signed(origin)?;

			let proxy_def =
				Proxies::<T>::get(&delegator, &delegate).ok_or(Error::<T>::NoSuchProxy)?;

			Proxies::<T>::remove(&delegator, &delegate);

			match proxy_def.sponsor {
				Some(sponsor) => {
					T::Currency::release(
						&HoldReason::ProxyDeposit.into(),
						&sponsor,
						T::ProxyDeposit::get(),
						Precision::Exact,
					)?;
				},
				None => {
					T::Currency::release(
						&HoldReason::ProxyDeposit.into(),
						&delegator,
						T::ProxyDeposit::get(),
						Precision::Exact,
					)?;
				},
			}

			Self::deposit_event(Event::ProxyRemoved {
				delegator,
				delegate,
				removed_by_sponsor: None,
			});

			Ok(())
		}

		/// Approves funding for a proxy.
		///
		/// This extrinsic allows a sponsor agent to approve the reservation of funds for a proxy on behalf
		/// of the sponsor. The approval must be given before the proxy can be created using the sponsor's funds.
		///
		/// Emits `ProxySponsorshipApproved` event.
		///
		/// # Parameters
		/// - `origin`: The sponsor agent's account.
		/// - `sponsor`: The sponsor's account that will reserve the funds.
		/// - `delegator`: The delegator's account that will use the sponsor's funds.
		///
		/// # Errors
		/// - `SponsorAgentUnauthorized`: If the caller is not an authorized agent of the sponsor.
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::approve_proxy_funding())]
		pub fn approve_proxy_funding(
			origin: OriginFor<T>,
			sponsor: T::AccountId,
			delegator: T::AccountId,
		) -> DispatchResult {
			let approver = ensure_signed(origin)?;

			ensure!(
				approver == sponsor || Self::has_sponsor_agent(&sponsor, &approver),
				Error::<T>::SponsorAgentUnauthorized
			);

			Self::add_approval(&delegator, &sponsor, &approver);

			Self::deposit_event(Event::ProxySponsorshipApproved { delegator, sponsor, approver });

			Ok(())
		}

		/// Registers a sponsor agent.
		///
		/// This extrinsic allows a sponsor to register an agent who is authorized to approve the reservation
		/// of funds for proxies on behalf of the sponsor. This helps in delegating the responsibility of
		/// managing proxy fund reservations while keeping the sponsor's credentials secure.
		///
		/// Emits `SponsorAgentRegistered` event.
		///
		/// # Parameters
		/// - `origin`: The sponsor's account.
		/// - `sponsor_agent`: The account to be registered as the sponsor's agent.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::register_sponsor_agent())]
		pub fn register_sponsor_agent(
			origin: OriginFor<T>,
			sponsor_agent: T::AccountId,
		) -> DispatchResult {
			let sponsor = ensure_signed(origin)?;

			ensure!(
				!SponsorAgents::<T>::contains_key(&sponsor_agent),
				Error::<T>::SponsorAgentAlreadyRegistered
			);

			SponsorAgents::<T>::insert(&sponsor_agent, &sponsor);

			Self::deposit_event(Event::SponsorAgentRegistered { sponsor, agent: sponsor_agent });

			Ok(())
		}

		/// Revokes a sponsor agent.
		///
		/// Revokes the authorization of a sponsor agent. Once revoked, the agent will no longer be able
		/// to approve the reservation of funds for proxies on behalf of the sponsor.
		/// All previously approved fund reservations by this agent that have not yet been used to create proxies will also be invalidated.
		/// Existing proxies created with the agent's approval will remain unaffected.
		///
		/// Emits `SponsorAgentRevoked` event.
		///
		/// # Parameters
		/// - `origin`: The sponsor's account.
		/// - `sponsor_agent`: The account to be revoked as the sponsor's agent.
		///
		/// # Errors
		/// - `SponsorAgentUnauthorized`: If the specified agent is not currently authorized by the sponsor.
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::revoke_sponsor_agent())]
		pub fn revoke_sponsor_agent(
			origin: OriginFor<T>,
			sponsor_agent: T::AccountId,
		) -> DispatchResult {
			let sponsor = ensure_signed(origin)?;

			ensure!(Self::has_sponsor_agent(&sponsor, &sponsor_agent), Error::<T>::Unauthorized);

			SponsorAgents::<T>::remove(&sponsor_agent);
			InvalidatedAgents::<T>::insert(&sponsor_agent, ());

			Self::deposit_event(Event::SponsorAgentRevoked { sponsor, agent: sponsor_agent });

			Ok(())
		}

		/// Removes a proxy sponsored by the caller.
		///
		/// This extrinsic allows a sponsor to remove a proxy that they have sponsored. The reserved deposit
		/// is returned to the sponsor upon removal of the proxy.
		///
		/// Emits `ProxyRemoved` event.
		///
		/// # Parameters
		/// - `origin`: The sponsor's account.
		/// - `delegator`: The account that delegated its authority.
		/// - `delegate`: The account that received the delegation.
		///
		/// # Errors
		/// - `NoSuchProxy`: If the proxy does not exist.
		/// - `Unauthorized`: If the caller is not the sponsor of the specified proxy.
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::remove_sponsored_proxy())]
		pub fn remove_sponsored_proxy(
			origin: OriginFor<T>,
			delegator: T::AccountId,
			delegate: T::AccountId,
		) -> DispatchResult {
			let sponsor = ensure_signed(origin)?;

			let proxy_def =
				Proxies::<T>::get(&delegator, &delegate).ok_or(Error::<T>::NoSuchProxy)?;

			ensure!(proxy_def.sponsor == Some(sponsor.clone()), Error::<T>::Unauthorized);

			Proxies::<T>::remove(&delegator, &delegate);

			T::Currency::release(
				&HoldReason::ProxyDeposit.into(),
				&sponsor,
				T::ProxyDeposit::get(),
				Precision::Exact,
			)?;

			Self::deposit_event(Event::ProxyRemoved {
				delegator,
				delegate,
				removed_by_sponsor: Some(sponsor),
			});

			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_idle(_n: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
			let mut meter = WeightMeter::with_limit(remaining_weight);

			while meter.can_consume(T::WeightInfo::cleanup_approvals()) {
				if !Self::cleanup_approvals() {
					break;
				}
				meter.consume(T::WeightInfo::cleanup_approvals());
			}

			meter.consumed()
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn has_proxy(delegator: &T::AccountId, delegate: &T::AccountId) -> bool {
		Proxies::<T>::contains_key(delegator, delegate)
	}

	pub fn has_sponsor_agent(sponsor: &T::AccountId, sponsor_agent: &T::AccountId) -> bool {
		SponsorAgents::<T>::get(sponsor_agent) == Some(sponsor.clone())
	}

	pub fn has_sponsorship_approval(delegator: &T::AccountId, sponsor: &T::AccountId) -> bool {
		let maybe_approver = SponsorshipApprovals::<T>::get(&(delegator.clone(), sponsor.clone()));

		match maybe_approver {
			Some(approver) => approver == *sponsor || Self::has_sponsor_agent(sponsor, &approver),
			None => false,
		}
	}

	fn add_approval(delegator: &T::AccountId, sponsor: &T::AccountId, agent: &T::AccountId) {
		let approval_key = (delegator.clone(), sponsor.clone());

		SponsorshipApprovals::<T>::insert(&approval_key, agent.clone());
		ApprovalsByAgent::<T>::insert(agent, &approval_key, ());
	}

	fn remove_approval(delegator: &T::AccountId, sponsor: &T::AccountId) {
		let approval_key = (delegator.clone(), sponsor.clone());

		let maybe_agent = SponsorshipApprovals::<T>::get(&approval_key);

		if let Some(agent) = maybe_agent {
			ApprovalsByAgent::<T>::remove(&agent, &(delegator.clone(), sponsor.clone()));
		}

		SponsorshipApprovals::<T>::remove(&(delegator.clone(), sponsor.clone()));
	}

	/// Clean up approvals that are no longer valid because the agent has been removed.
	/// Returns `true` if an approval was removed, `false` otherwise.
	///
	/// This function only removes one approval at a time to avoid blocking the runtime and
	/// to ease calculation of the consumed weight.
	pub fn cleanup_approvals() -> bool {
		InvalidatedAgents::<T>::iter().next().map_or(false, |(agent, _)| {
			let maybe_approval = ApprovalsByAgent::<T>::iter_prefix(&agent).next();

			if let Some((approval, _)) = maybe_approval {
				let (delegator, sponsor) = approval;

				Self::remove_approval(&delegator, &sponsor);

				true
			} else {
				InvalidatedAgents::<T>::remove(&agent);

				false
			}
		})
	}
}
