use std::error::Error;

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::Signer,
};

use crate::{Miner, utils::proof_pubkey};

impl Miner {
    pub async fn register(&self) -> Result<(), Box<dyn Error>> {
        // Return early if miner is already registered
        let signer = self.signer();
        let proof_address = proof_pubkey(signer.pubkey());
        let client =
            RpcClient::new_with_commitment(self.cluster.clone(), CommitmentConfig::finalized());
        if client.get_account(&proof_address).await.is_ok() {
            return Ok(());
        }

        // Sign and send transaction.
        println!("Generating challenge...");
        let ix = ore::instruction::register(signer.pubkey());
        match self.send_and_confirm(&[ix], false)
            .await {
            Ok(_) =>
                {
                    println!("Registration successful");
                    Ok(())
                }
            Err(e) => {
                eprintln!("Registration failed: {:?}", e);
                return Err(e.into());
            }
        }
    }
}
