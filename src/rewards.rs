use std::str::FromStr;

use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;

use crate::{Miner, utils::get_proof};

impl Miner {
    pub async fn rewards(&self, address: Option<String>) {
        let address = if let Some(address) = address {
            if let Ok(address) = Pubkey::from_str(&address) {
                address
            } else {
                println!("Invalid address: {:?}", address);
                return;
            }
        } else {
            self.signer().pubkey()
        };
        let proof = match get_proof(self.cluster.clone(), address).await {
            Ok(proof) => proof,
            Err(e) => {
                println!("Failed to get proof: {:?}", e);
                return;
            }
        };
        let amount = (proof.claimable_rewards as f64) / 10f64.powf(ore::TOKEN_DECIMALS as f64);
        println!("{:} ORE", amount);
    }
}
