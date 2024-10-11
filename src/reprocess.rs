use coal_utils::AccountDeserialize;
use solana_client::nonblocking::rpc_client::RpcClient;
use tokio::time::{sleep, Duration};

use colored::*;
use solana_sdk::signer::Signer;
use solana_program::pubkey::Pubkey;
use coal_api::{consts::REPROCESSOR, state::Reprocessor};


use crate::{
    args::ReprocessArgs,
    send_and_confirm::ComputeBudget,
    Miner,
};

fn get_reprocessor_address(signer: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[REPROCESSOR, signer.as_ref()], &coal_api::id()).0
}

async fn get_reprocessor(client: &RpcClient, signer: &Pubkey) -> Option<Reprocessor> {
    let address = get_reprocessor_address(&signer);
    let account_data = client.get_account_data(&address).await;

    if let Ok(account_data) = account_data {
        Some(*Reprocessor::try_from_bytes(&account_data).unwrap())
    } else {
        None
    }
    
}

impl Miner {
    pub async fn reprocess(&self, _args: ReprocessArgs) {
		let signer = self.signer();
        println!("{} {}", "INFO".bold().green(), "Reprocessing...");
        let mut reprocessor = get_reprocessor(&self.rpc_client, &signer.pubkey()).await;

        if reprocessor.is_none() {
            let ix = coal_api::instruction::init_reprocess(signer.pubkey());
            let res = self.send_and_confirm(&[ix], ComputeBudget::Fixed(100_000), false).await.ok();
            println!("{:?}", res);
            sleep(Duration::from_secs(5)).await;
            reprocessor = get_reprocessor(&self.rpc_client, &signer.pubkey()).await;
        }

        let target_slot = reprocessor.unwrap().slot + 10;
        // TODO: get reprocessor account and target slot

        println!("{} {}", "INFO".bold().yellow(), format!("Waiting for slot {}...", target_slot));
        
        loop {
            match self.rpc_client.get_slot().await {
                Ok(current_slot) => {
                    if current_slot >= target_slot - 1 {
                        println!("{} {}", "INFO".bold().green(), format!("Target slot {} reached", target_slot));
                        let ix = coal_api::instruction::finalize_reprocess(signer.pubkey());
                        let res = self.send_and_confirm(&[ix], ComputeBudget::Fixed(200_000), false).await.ok();
                        println!("{:?}", res);
                        break;
                    }
                    sleep(Duration::from_millis(400)).await;
                },
                Err(e) => {
                    println!("{} {}", "ERROR".bold().red(), format!("Failed to get current slot: {}", e));
                    sleep(Duration::from_secs(400)).await;
                }
            }
        }
        println!("{} {}", "INFO".bold().green(), "Reprocessed");
	}
}