/*
	Copyright 2021 Integritee AG and Supercomputing Systems AG

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

use crate::{
	get_layer_two_nonce,
	trusted_cli::TrustedCli,
	trusted_command_utils::{get_identifiers, get_pair_from_str},
	trusted_operation::perform_trusted_operation,
	Cli, CliResult, CliResultOk,
};
use ita_stf::{Index, RaffleIndex, RaffleTrustedCall, TrustedCall};
use itp_stf_primitives::{
	traits::TrustedCallSigning,
	types::{KeyPair, TrustedOperation},
};
use itp_types::AccountId;
use log::*;
use sp_core::{crypto::Ss58Codec, Pair};
use std::boxed::Box;

#[derive(Debug, Parser)]
pub struct RegisterForRaffleCmd {
	/// Sender's incognito AccountId in ss58check format
	from: String,

	/// Winner count of hte raffle
	raffle_index: RaffleIndex,
}

impl RegisterForRaffleCmd {
	pub(crate) fn run(&self, cli: &Cli, trusted_args: &TrustedCli) -> CliResult {
		let sender = get_pair_from_str(trusted_args, &self.from);
		let sender_acc: AccountId = sender.public().into();
		info!("senders ss58 is {}", sender.public().to_ss58check());

		let (mrenclave, shard) = get_identifiers(trusted_args);
		let nonce = get_layer_two_nonce!(sender, cli, trusted_args);

		let add_raffle_call = RaffleTrustedCall::registerForRaffle {
			origin: sender_acc,
			raffle_index: self.raffle_index,
		};

		let function_call = TrustedCall::raffle(add_raffle_call)
			.sign(&KeyPair::Sr25519(Box::new(sender)), nonce, &mrenclave, &shard)
			.into_trusted_operation(trusted_args.direct);
		Ok(perform_trusted_operation::<()>(cli, trusted_args, &function_call)
			.map(|_| CliResultOk::None)?)
	}
}
