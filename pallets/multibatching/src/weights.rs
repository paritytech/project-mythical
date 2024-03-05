use frame_support::weights::Weight;

// TODO

/// Weight functions needed for pallet_template.
pub trait WeightInfo {
	fn batch() -> Weight;
}

// TODO
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn batch() -> Weight {
        Weight::default()
    }
}
