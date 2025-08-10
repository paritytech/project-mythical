#![cfg(test)]

use super::*;
use crate::mock::*;
use account::AccountId20;
use frame_support::{assert_ok, traits::{Currency, Hooks}};

fn account(id: u8) -> AccountId20 {
	[id; 20].into()
}

mod testing_utilities {
	use super::*;

	#[test]
	/// Call `transfer_through_delayed_remint`, ensure that the transfer
	/// has succeeded and all the following events are emitted:
	///
	/// * `pallet_testing_utilities::Event::Scheduled`
	/// * `pallet_balances::Event::Burned`
	/// * `pallet_balances::Event::Minted`
	/// * `pallet_testing_utilities::Event::Executed`
	fn should_make_transfer_with_detached_events() {
		new_test_ext().execute_with(|| {
			let from = account(1);
			let to = account(2);
			let amount = 9_001;

			Balances::make_free_balance_be(&from, 10_000);
			Balances::make_free_balance_be(&to,       10);

			// Implement the functionality to test the transfer and event emissions
			assert_ok!(Pallet::<Test>::transfer_through_delayed_remint(
				RuntimeOrigin::signed(from),
				to,
				amount,
			));

			// Verify the events were emitted
			System::assert_last_event(Event::Scheduled.into());

			System::run_to_block::<AllPalletsWithSystem>(2_u64.into());

			// `run_to_block` does not call `on_idle`, so call it manually.
			TestingUtilities::on_idle(2_u64.into(), Weight::MAX);

			System::assert_has_event(pallet_balances::Event::Burned { who: from, amount }.into());
			System::assert_has_event(pallet_balances::Event::Minted { who: to, amount }.into());
			System::assert_last_event(Event::Executed { scheduled_in: 1 }.into());
		});
	}
}
