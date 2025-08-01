use core::marker::PhantomData;
use frame_support::weights::Weight;

pub trait WeightInfo {
	fn transfer_through_delayed_remint() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn transfer_through_delayed_remint() -> Weight {
		Weight::zero() // TODO
	}
}
