#[cfg(all(feature = "std", feature = "metadata-hash"))]
fn main() {
	substrate_wasm_builder::WasmBuilder::init_with_defaults()
		.enable_metadata_hash("MYTH", 18)
		.build()
}

#[cfg(feature = "std")]
fn main() {
	substrate_wasm_builder::WasmBuilder::init_with_defaults().build()
}

/// The wasm builder is deactivated when compiling
/// this crate for wasm to speed up the compilation.
#[cfg(not(feature = "std"))]
fn main() {}
