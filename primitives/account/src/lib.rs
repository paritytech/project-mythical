// Copyright 2019-2022 PureStake Inc.
// This file is part of Moonbeam.

// Moonbeam is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Moonbeam is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Moonbeam.  If not, see <http://www.gnu.org/licenses/>.

//! The Ethereum Signature implementation.
//!
//! It includes the Verify and IdentifyAccount traits for the AccountId20

#![cfg_attr(not(feature = "std"), no_std)]

use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode, Error, Input, MaxEncodedLen};
use scale_info::TypeInfo;
use sha3::{Digest, Keccak256};
use sp_core::{ecdsa, H160};

pub use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sp_core::crypto::{AccountId32, FromEntropy};
#[cfg(feature = "std")]
use sp_io::hashing::keccak_256;
use sp_runtime::MultiSignature;

/// The account type to be used in Moonbeam. It is a wrapper for 20 fixed bytes. We prefer to use
/// a dedicated type to prevent using arbitrary 20 byte arrays were AccountIds are expected. With
/// the introduction of the `scale-info` crate this benefit extends even to non-Rust tools like
/// Polkadot JS.
#[derive(
	Eq,
	PartialEq,
	Copy,
	Clone,
	Encode,
	Decode,
	DecodeWithMemTracking,
	TypeInfo,
	MaxEncodedLen,
	Default,
	PartialOrd,
	Ord,
)]
pub struct AccountId20(pub [u8; 20]);

impl_serde::impl_fixed_hash_serde!(AccountId20, 20);

#[cfg(feature = "std")]
impl std::fmt::Display for AccountId20 {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let address = hex::encode(self.0).trim_start_matches("0x").to_lowercase();
		let address_hash = hex::encode(keccak_256(address.as_bytes()));

		let checksum: String =
			address
				.char_indices()
				.fold(String::from("0x"), |mut acc, (index, address_char)| {
					let n = u16::from_str_radix(&address_hash[index..index + 1], 16)
						.expect("Keccak256 hashed; qed");

					if n > 7 {
						// make char uppercase if ith character is 9..f
						acc.push_str(&address_char.to_uppercase().to_string())
					} else {
						// already lowercased
						acc.push(address_char)
					}

					acc
				});
		write!(f, "{checksum}")
	}
}

impl core::fmt::Debug for AccountId20 {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		write!(f, "{:?}", H160(self.0))
	}
}

impl From<[u8; 20]> for AccountId20 {
	fn from(bytes: [u8; 20]) -> Self {
		Self(bytes)
	}
}

impl From<AccountId20> for [u8; 20] {
	fn from(value: AccountId20) -> Self {
		value.0
	}
}

// NOTE: the implementation is lossy, and is intended to be used
// only to convert from Polkadot accounts to AccountId20.
// See https://github.com/moonbeam-foundation/moonbeam/pull/2315#discussion_r1205830577
// DO NOT USE IT FOR ANYTHING ELSE.
impl From<[u8; 32]> for AccountId20 {
	fn from(bytes: [u8; 32]) -> Self {
		let mut buffer = [0u8; 20];
		buffer.copy_from_slice(&bytes[..20]);
		Self(buffer)
	}
}

impl From<H160> for AccountId20 {
	fn from(h160: H160) -> Self {
		Self(h160.0)
	}
}

impl From<AccountId20> for H160 {
	fn from(value: AccountId20) -> Self {
		H160(value.0)
	}
}

impl From<AccountId32> for AccountId20 {
	fn from(account: AccountId32) -> Self {
		let bytes: &[u8; 32] = account.as_ref();
		Self::from(*bytes)
	}
}

#[cfg(feature = "std")]
impl std::str::FromStr for AccountId20 {
	type Err = &'static str;
	fn from_str(input: &str) -> Result<Self, Self::Err> {
		H160::from_str(input).map(Into::into).map_err(|_| "invalid hex address.")
	}
}

/// Creates an [`AccountId20`] from the input, which should contain at least 20 bytes.
impl FromEntropy for AccountId20 {
	fn from_entropy(input: &mut impl Input) -> Result<Self, Error> {
		let entropy: [u8; 20] = FromEntropy::from_entropy(input)?;
		Ok(AccountId20::from(entropy))
	}
}

#[derive(
	Eq,
	PartialEq,
	Clone,
	Encode,
	Decode,
	DecodeWithMemTracking,
	sp_core::RuntimeDebug,
	TypeInfo,
	Serialize,
	Deserialize,
)]
pub struct EthereumSignature(ecdsa::Signature);

impl From<ecdsa::Signature> for EthereumSignature {
	fn from(x: ecdsa::Signature) -> Self {
		Self(x)
	}
}

impl sp_runtime::traits::Verify for EthereumSignature {
	type Signer = EthereumSigner;
	fn verify<L: sp_runtime::traits::Lazy<[u8]>>(&self, mut msg: L, signer: &AccountId20) -> bool {
		let mut m = [0u8; 32];
		// Here we use the sha256 hashing algorithm instead of the expected blake2_256.
		// The reason is that we intend to verify Ethereum signatures, which expect a keccak256
		// digest instead of the expected blake2_256 one in secp256k1_ecdsa_recover.
		m.copy_from_slice(Keccak256::digest(msg.get()).as_slice());
		match sp_io::crypto::secp256k1_ecdsa_recover(self.0.as_ref(), &m) {
			Ok(pubkey) => {
				AccountId20(H160::from_slice(&Keccak256::digest(pubkey).as_slice()[12..32]).0)
					== *signer
			},
			Err(sp_io::EcdsaVerifyError::BadRS) => {
				log::error!(target: "evm", "Error recovering: Incorrect value of R or S");
				false
			},
			Err(sp_io::EcdsaVerifyError::BadV) => {
				log::error!(target: "evm", "Error recovering: Incorrect value of V");
				false
			},
			Err(sp_io::EcdsaVerifyError::BadSignature) => {
				log::error!(target: "evm", "Error recovering: Invalid signature");
				false
			},
		}
	}
}

impl From<MultiSignature> for EthereumSignature {
	fn from(signature: MultiSignature) -> Self {
		match signature {
			MultiSignature::Ed25519(_) => {
				panic!("Ed25519 not supported for EthereumSignature")
			},
			MultiSignature::Sr25519(_) => {
				panic!("Sr25519 not supported for EthereumSignature")
			},
			MultiSignature::Ecdsa(sig) => Self(sig),
		}
	}
}

/// Public key for an Ethereum / Moonbeam compatible account
#[derive(
	Eq,
	PartialEq,
	Ord,
	PartialOrd,
	Clone,
	Encode,
	Decode,
	DecodeWithMemTracking,
	sp_core::RuntimeDebug,
	TypeInfo,
)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct EthereumSigner([u8; 20]);

impl sp_runtime::traits::IdentifyAccount for EthereumSigner {
	type AccountId = AccountId20;
	fn into_account(self) -> AccountId20 {
		AccountId20(self.0)
	}
}

impl From<[u8; 20]> for EthereumSigner {
	fn from(x: [u8; 20]) -> Self {
		EthereumSigner(x)
	}
}

impl From<ecdsa::Public> for EthereumSigner {
	fn from(x: ecdsa::Public) -> Self {
		let decompressed = libsecp256k1::PublicKey::parse_compressed(&x.0)
			.expect("Wrong compressed public key provided")
			.serialize();
		let mut m = [0u8; 64];
		m.copy_from_slice(&decompressed[1..65]);
		let account = H160::from_slice(&Keccak256::digest(m).as_slice()[12..32]);
		EthereumSigner(account.into())
	}
}

impl From<libsecp256k1::PublicKey> for EthereumSigner {
	fn from(x: libsecp256k1::PublicKey) -> Self {
		let mut m = [0u8; 64];
		m.copy_from_slice(&x.serialize()[1..65]);
		let account = H160::from_slice(&Keccak256::digest(m).as_slice()[12..32]);
		EthereumSigner(account.into())
	}
}

#[cfg(feature = "std")]
impl std::fmt::Display for EthereumSigner {
	fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(fmt, "ethereum signer: {:?}", H160::from_slice(&self.0))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use hex::ToHex;
	use sp_core::hexdisplay::AsBytesRef;
	use sp_core::{Pair, H256};
	use sp_runtime::traits::{IdentifyAccount, Verify};

	#[test]
	fn test_account_derivation_1() {
		// Test from https://asecuritysite.com/encryption/ethadd
		let secret_key =
			hex::decode("502f97299c472b88754accd412b7c9a6062ef3186fba0c0388365e1edec24875")
				.unwrap();
		let mut expected_hex_account = [0u8; 20];
		hex::decode_to_slice("976f8456e4e2034179b284a23c0e0c8f6d3da50c", &mut expected_hex_account)
			.expect("example data is 20 bytes of valid hex");

		let public_key = ecdsa::Pair::from_seed_slice(&secret_key).unwrap().public();
		let account: EthereumSigner = public_key.into();
		let expected_account = AccountId20::from(expected_hex_account);
		assert_eq!(account.into_account(), expected_account);
	}
	#[test]
	fn test_account_derivation_2() {
		// Test from https://asecuritysite.com/encryption/ethadd
		let secret_key =
			hex::decode("0f02ba4d7f83e59eaa32eae9c3c4d99b68ce76decade21cdab7ecce8f4aef81a")
				.unwrap();
		let mut expected_hex_account = [0u8; 20];
		hex::decode_to_slice("420e9f260b40af7e49440cead3069f8e82a5230f", &mut expected_hex_account)
			.expect("example data is 20 bytes of valid hex");

		let public_key = ecdsa::Pair::from_seed_slice(&secret_key).unwrap().public();
		let account: EthereumSigner = public_key.into();
		let expected_account = AccountId20::from(expected_hex_account);
		assert_eq!(account.into_account(), expected_account);
	}
	#[test]
	fn test_account_derivation_3() {
		let m = hex::decode("c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470")
			.unwrap();
		let old = AccountId20(H160::from(H256::from_slice(Keccak256::digest(&m).as_slice())).0);
		let new = AccountId20(H160::from_slice(&Keccak256::digest(&m).as_slice()[12..32]).0);
		assert_eq!(new, old);
	}
	#[test]
	fn test_verify() {
		let raw_message = b"Hello, world!";
		let message_hash = Keccak256::digest(raw_message.as_bytes_ref());
		assert_eq!(
			message_hash.encode_hex::<String>(),
			"b6e16d27ac5ab427a7f68900ac5559ce272dc6c37c82b3e052246c82244c50e4"
		);

		let private_key = "0xbbeca31142ae5cf58522af17442bd8a63b1aa7c485b266ce5256b6e2d6fb8fda";
		let pair = ecdsa::Pair::from_string(private_key, None).expect("Invalid private key");
		let ecdsa_signature = pair.sign_prehashed(message_hash.as_ref());
		assert_eq!(ecdsa_signature.encode_hex::<String>(), "33f6526244820bf440604df9d2638461ee09477fa6b01c80321f63b9c9995a5971f7e28e9c6aeb52a89d605fb07f13e59cc8ec352ac78ebbc75af0543c1b3d3400");

		let signer: EthereumSigner = pair.public().into();
		let account = signer.into_account();
		assert_eq!(
			account.0.encode_hex::<String>(),
			"C80AfE84e5E9e37bDF58855E5A0cB17a2bC70cfA".to_lowercase()
		);

		let ethereum_signature: EthereumSignature = ecdsa_signature.into();
		let result = ethereum_signature.verify(raw_message.as_bytes_ref(), &account);
		assert!(result);
	}
}
