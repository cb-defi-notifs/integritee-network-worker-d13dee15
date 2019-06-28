/*
	Copyright 2019 Supercomputing Systems AG

	Licensed under the Apache License, Version 2.0 (the "License");
	you may not use this file except in compliance with the License.
	You may obtain a copy of the License at

		http://www.apache.org/licenses/LICENSE-2.0

	Unless required by applicable law or agreed to in writing, software
	distributed under the License is distributed on an "AS IS" BASIS,
	WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
	See the License for the specific language governing permissions and
	limitations under the License.

*/
extern crate aes;
extern crate ofb;
extern crate sgx_types;

use blake2_no_std::blake2b::blake2b;
use crypto::blake2s::Blake2s;
use log::*;
use my_node_runtime::Hash;
use sgx_crypto_helper::rsa3072::{Rsa3072KeyPair, Rsa3072PubKey};
use sgx_crypto_helper::RsaKeyPair;
use sgx_rand::{Rng, StdRng};
use sgx_types::*;

use constants::{AES_KEY_FILE_AND_INIT_V, COUNTERSTATE, ED25519_SEALED_KEY_FILE, RSA3072_SEALED_KEY_FILE};
use std::fs::File;
use std::io::{Read, Write};
use std::sgxfs::SgxFile;
use std::vec::Vec;

use self::aes::Aes128;
use self::ofb::Ofb;
use self::ofb::stream_cipher::{NewStreamCipher, SyncStreamCipher};

type AesOfb = Ofb<Aes128>;

pub fn read_rsa_keypair() -> SgxResult<Rsa3072KeyPair> {
	let keyvec = read_file(RSA3072_SEALED_KEY_FILE)?;
	let key_json_str = std::str::from_utf8(&keyvec).unwrap();
	let pair: Rsa3072KeyPair = serde_json::from_str(&key_json_str).unwrap();
	Ok(pair)
}

pub fn read_rsa_pubkey() -> SgxResult<Rsa3072PubKey> {
	let pair = r#try!(read_rsa_keypair());
	let pubkey = pair.export_pubkey().unwrap();

	Ok(pubkey)
}

pub fn get_ecc_seed() -> SgxResult<Vec<u8>> {
	read_file(ED25519_SEALED_KEY_FILE)
}

pub fn create_sealed_ed25519_seed() -> SgxResult<sgx_status_t> {
	let mut seed = [0u8; 32];
	let mut rand = match StdRng::new() {
		Ok(rng) => rng,
		Err(_) => { return Err(sgx_status_t::SGX_ERROR_UNEXPECTED); },
	};
	rand.fill_bytes(&mut seed);

	write_file(&seed, ED25519_SEALED_KEY_FILE)
}

pub fn read_aes_key_and_iv() -> SgxResult<Vec<u8>> {
	read_file(AES_KEY_FILE_AND_INIT_V)
}

pub fn create_sealed_aes_key_and_iv() -> SgxResult<sgx_status_t> {
	let mut key_iv = [0u8; 32];

	let mut rand = match StdRng::new() {
		Ok(rng) => rng,
		Err(_) => { return Err(sgx_status_t::SGX_ERROR_UNEXPECTED); },
	};

	rand.fill_bytes(&mut key_iv);
	write_file(&key_iv, AES_KEY_FILE_AND_INIT_V)
}

pub fn write_file(bytes: &[u8], filepath: &str) -> SgxResult<sgx_status_t> {
	match SgxFile::create(filepath) {
		Ok(mut f) => match f.write_all(bytes) {
			Ok(()) => {
				info!("[Enclave] Writing keyfile '{}' successful", filepath);
				Ok(sgx_status_t::SGX_SUCCESS)
			}
			Err(x) => {
				error!("[Enclave -] Writing keyfile '{}' failed! {}", filepath, x);
				Err(sgx_status_t::SGX_ERROR_UNEXPECTED)
			}
		},
		Err(x) => {
			error!("[Enclave !] Creating keyfile '{}' error! {}", filepath, x);
			Err(sgx_status_t::SGX_ERROR_UNEXPECTED)
		}
	}
}

pub fn read_file(filepath: &str) -> SgxResult<Vec<u8>> {
	let mut keyvec: Vec<u8> = Vec::new();
	match SgxFile::open(filepath) {
		Ok(mut f) => match f.read_to_end(&mut keyvec) {
			Ok(len) => {
				info!("[Enclave] Read {} bytes from key file", len);
				Ok(keyvec)
			}
			Err(x) => {
				error!("[Enclave] Read key file failed {}", x);
				Err(sgx_status_t::SGX_ERROR_UNEXPECTED)
			}
		},
		Err(x) => {
			error!("[Enclave] get_sealed_pcl_key cannot open key file, please check if key is provisioned successfully! {}", x);
			Err(sgx_status_t::SGX_ERROR_UNEXPECTED)
		}
	}
}

pub fn read_counterfile() -> SgxResult<Vec<u8>> {
	let mut buffer = read_plaintext(COUNTERSTATE)?;

	let key_iv = read_aes_key_and_iv()?;
	AesOfb::new_var(&key_iv[..16], &key_iv[16..]).unwrap().apply_keystream(&mut buffer);
	println!("buffer decrypted = {:?}", buffer);

	Ok(buffer)
}

pub fn write_counterfile(mut bytes: Vec<u8>) -> SgxResult<sgx_status_t> {
	println!("data to be written: {:?}", bytes);

	let key_iv = read_aes_key_and_iv()?;
	AesOfb::new_var(&key_iv[..16], &key_iv[16..]).unwrap().apply_keystream(&mut bytes);

	write_plaintext(&bytes, COUNTERSTATE)
}

pub fn read_plaintext(filepath: &str) -> SgxResult<Vec<u8>> {
	let mut state_vec: Vec<u8> = Vec::new();
	match File::open(filepath) {
		Ok(mut f) => match f.read_to_end(&mut state_vec) {
			Ok(len) => {
				info!("[Enclave] Read {} bytes from counter file", len);
				Ok(state_vec)
			}
			Err(x) => {
				error!("[Enclave] Read counter file failed {}", x);
				Err(sgx_status_t::SGX_ERROR_UNEXPECTED)
			}
		},
		Err(x) => {
			error!("[Enclave] can't get counter file, initializing new Counter! {}", x);
			state_vec.push(0);
			Ok(state_vec)
		}
	}
}

pub fn write_plaintext(bytes: &[u8], filepath: &str) -> SgxResult<sgx_status_t> {
	match File::create(filepath) {
		Ok(mut f) => match f.write_all(bytes) {
			Ok(()) => {
				info!("[Enclave] Writing to file '{}' successful", filepath);
				Ok(sgx_status_t::SGX_SUCCESS)
			}
			Err(x) => {
				error!("[Enclave] Writing to '{}' failed! {}", filepath, x);
				Err(sgx_status_t::SGX_ERROR_UNEXPECTED)
			}
		},
		Err(x) => {
			error!("[Enclave] Creating file '{}' error! {}", filepath, x);
			Err(sgx_status_t::SGX_ERROR_UNEXPECTED)
		}
	}
}

pub fn decode_payload(ciphertext_slice: &[u8], rsa_pair: &Rsa3072KeyPair) -> Vec<u8> {
	let mut decrypted_buffer = Vec::new();
	rsa_pair.decrypt_buffer(ciphertext_slice, &mut decrypted_buffer).unwrap();
	decrypted_buffer
}

pub fn hash_from_slice(hash_slize: &[u8]) -> Hash {
	let mut g = [0; 32];
	g.copy_from_slice(&hash_slize[..]);
	Hash::from(&mut g)
}

pub fn blake2s(plaintext: &[u8]) -> [u8; 32] {
	let mut call_hash: [u8; 32] = Default::default();
	Blake2s::blake2s(&mut call_hash, &plaintext[..], &[0; 32]);
	call_hash
}

// Same functions as in substrate/core/primitives, but using the no_std blake2_rfc
/// Do a Blake2 256-bit hash and place result in `dest`.
fn blake2_256_into(data: &[u8], dest: &mut [u8; 32]) {
	dest.copy_from_slice(blake2b(32, &[], data).as_bytes());
}

/// Do a Blake2 256-bit hash and return result.
pub fn blake2_256(data: &[u8]) -> [u8; 32] {
	let mut r = [0; 32];
	blake2_256_into(data, &mut r);
	r
}

pub fn test_counterstate_io_works() {
	let plaintext = b"The quick brown fox jumps over the lazy dog.";
	create_sealed_aes_key_and_iv();

	write_counterfile(plaintext.to_vec());
	let mut state: Vec<u8> = read_counterfile().unwrap();
	assert_eq!(state, plaintext.to_vec());
}

