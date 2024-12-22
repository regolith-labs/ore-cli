use std::str::FromStr;

use ore_api::consts::TOKEN_DECIMALS;
use ore_boost_api::state::reservation_pda;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use spl_token::amount_to_ui_amount;

use crate::{
    args::ProofArgs,
    utils::{get_proof, proof_pubkey, get_reservation},
    Miner,
};

impl Miner {
    pub async fn proof(&self, args: ProofArgs) {
        let signer = self.signer();
        let proof_address = if let Some(address) = args.address {
            Pubkey::from_str(&address).unwrap()
        } else {
            proof_pubkey(signer.pubkey())
        };
        let reservation_address = reservation_pda(proof_address).0;
        let proof = get_proof(&self.rpc_client, proof_address).await;
        let reservation = get_reservation(&self.rpc_client, reservation_address).await.unwrap();
        println!("Address: {:?}", proof_address);
        println!("Authority: {:?}", proof.authority);
        println!(
            "Balance: {:?} ORE",
            amount_to_ui_amount(proof.balance, TOKEN_DECIMALS)
        );
        println!("Boost: {:?}", reservation.boost);
        println!(
            "Last hash: {}",
            solana_sdk::hash::Hash::new_from_array(proof.last_hash).to_string()
        );
        println!("Last hash at: {:?}", proof.last_hash_at);
        println!("Miner: {:?}", proof.miner);
        println!("Total hashes: {:?}", proof.total_hashes);
        println!(
            "Total rewards: {:?} ORE",
            amount_to_ui_amount(proof.total_rewards, TOKEN_DECIMALS)
        );
    }
}
