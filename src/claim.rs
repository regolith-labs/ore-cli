use std::str::FromStr;

use ore::{self, state::Proof, utils::AccountDeserialize};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::pubkey::Pubkey;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::Signer,
};

use crate::{
    Miner,
    utils::proof_pubkey,
};

impl Miner {
    pub async fn claim(&self, cluster: String, beneficiary: Option<String>, amount: Option<f64>, retry_count: u64) {
        let signer = self.signer();
        let pubkey = signer.pubkey();
        let client = RpcClient::new_with_commitment(cluster, CommitmentConfig::finalized());

        let beneficiary = match beneficiary {
            Some(beneficiary) => {
                Pubkey::from_str(&beneficiary).expect("Failed to parse beneficiary address")
            }
            None => self.initialize_ata().await,
        };

        let amount = if let Some(amount) = amount {
            (amount * 10f64.powf(ore::TOKEN_DECIMALS as f64)) as u64
        } else {
            match client.get_account(&proof_pubkey(pubkey)).await {
                Ok(proof_account) => match Proof::try_from_bytes(&proof_account.data) {
                    Ok(proof) => proof.claimable_rewards,
                    Err(err) => {
                        eprintln!("Error parsing proof account data: {:?}", err);
                        return;
                    }
                },
                Err(err) => {
                    eprintln!("Error looking up claimable rewards: {:?}", err);
                    return;
                }
            }
        };

        let amountf = (amount as f64) / 10f64.powf(ore::TOKEN_DECIMALS as f64);

        let ix = ore::instruction::claim(pubkey, beneficiary, amount);

        for attempt in 0..=retry_count {
            println!("Attempt {}: Submitting claim transaction...", attempt + 1);
            let result = self
                .send_and_confirm(&[ix.clone()], false)
                .await;

            match result {
                Ok(sig) => {
                    println!("Claimed {:} ORE to account {:}", amountf, beneficiary);
                    println!("{:?}", sig);
                    return; // Transaction was successful, exit the loop and function
                }
                Err(err) => {
                    eprintln!("Error during transaction: {:?}", err);
                    // Optionally include a delay here if needed, e.g., tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
        eprintln!("Failed to claim after {} attempts", retry_count);
    }


    async fn initialize_ata(&self) -> Pubkey {
        // Initialize client.
        let signer = self.signer();
        let client =
            RpcClient::new_with_commitment(self.cluster.clone(), CommitmentConfig::finalized());

        // Build instructions.
        let token_account_pubkey = spl_associated_token_account::get_associated_token_address(
            &signer.pubkey(),
            &ore::MINT_ADDRESS,
        );

        // Check if ata already exists
        if let Ok(Some(_ata)) = client.get_token_account(&token_account_pubkey).await {
            return token_account_pubkey;
        }

        // Sign and send transaction.
        let ix = spl_associated_token_account::instruction::create_associated_token_account(
            &signer.pubkey(),
            &signer.pubkey(),
            &ore::MINT_ADDRESS,
            &spl_token::id(),
        );
        println!("Creating token account {}...", token_account_pubkey);
        match self
            .send_and_confirm(&[ix], false)
            .await
        {
            Ok(_sig) => println!("Created token account {:?}", token_account_pubkey),
            Err(e) => println!("Transaction failed: {:?}", e),
        }

        // Return token account address
        token_account_pubkey
    }
}
