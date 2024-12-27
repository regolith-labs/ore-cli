use std::str::FromStr;
use std::time::Duration;

use anyhow::Error;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use ore_boost_api::consts::CHECKPOINT_INTERVAL;  
use ore_boost_api::state::{boost_pda, checkpoint_pda};
use solana_rpc_client::spinner;
use colored::*;
use tokio::time::sleep;

use crate::{
    args::CheckpointArgs,
    send_and_confirm::ComputeBudget,
    Miner, utils::{get_clock, get_boost, get_checkpoint, get_stake_accounts},
};

const MAX_ACCOUNTS_PER_TX: usize = 10;

impl Miner {
    pub async fn checkpoint(&self, args: CheckpointArgs) -> Result<(), Error> {
        if args.continuous {
            self.checkpoint_continuous(&args).await?;
        } else {
            self.checkpoint_once(&args).await?;
        }
        Ok(())
    }

    async fn checkpoint_continuous(&self, args: &CheckpointArgs) -> Result<(), Error> {
        let mint_address = Pubkey::from_str(&args.mint)?;
        let boost_address = boost_pda(mint_address).0;
        let checkpoint_address = checkpoint_pda(boost_address).0;
        let progress_bar = spinner::new_progress_bar();
        loop {
            // Get current time and checkpoint data
            let clock = get_clock(&self.rpc_client).await;
            let checkpoint = get_checkpoint(&self.rpc_client, checkpoint_address).await;
            let time_since_last = clock.unix_timestamp - checkpoint.ts;

            // Call checkpoint if needed
            if time_since_last >= CHECKPOINT_INTERVAL {
                progress_bar.set_message("Checkpointing...");
                let _ = self.checkpoint_once(args).await;
            }

            // Sleep for 60 seconds
            progress_bar.set_message("Waiting...");
            sleep(Duration::from_secs(60)).await;
        }
    }

    async fn checkpoint_once(&self, args: &CheckpointArgs) -> Result<(), Error> {
        let progress_bar = spinner::new_progress_bar();
        progress_bar.set_message("Checkpointing...");

        // Parse mint address
        let mint_address = Pubkey::from_str(&args.mint)?;
        let boost_address = boost_pda(mint_address).0;
        let checkpoint_address = checkpoint_pda(boost_address).0;

        // Get boost account data
        let _boost = get_boost(&self.rpc_client, boost_address).await;
        let checkpoint = get_checkpoint(&self.rpc_client, checkpoint_address).await;

        // TODO Check if enough time has passed since last checkpoint
        let clock = get_clock(&self.rpc_client).await;
        let time_since_last = clock.unix_timestamp - checkpoint.ts;
        if time_since_last < CHECKPOINT_INTERVAL {
            progress_bar.finish_with_message(format!(
                "{} Not enough time has passed since last checkpoint. Wait {} more seconds.",
                "WARNING".yellow().bold(),
                CHECKPOINT_INTERVAL - time_since_last
            ));
            return Ok(());
        }

        // Get all stake accounts for this boost
        let mut accounts = get_stake_accounts(&self.rpc_client, boost_address).await?;
        if accounts.is_empty() {
            progress_bar.finish_with_message("No stake accounts found for this boost");
            return Ok(());
        }

        // Sort accounts by stake ID
        accounts.sort_by(|(_, stake_a), (_, stake_b)| {
            stake_a.id.cmp(&stake_b.id)
        });

        // Filter accounts starting from checkpoint.current_id
        let remaining_accounts: Vec<_> = accounts
            .into_iter()
            .filter(|(_, stake)| stake.id >= checkpoint.current_id)
            .collect();

        // Pack instructions for rebase
        let mut ixs = Vec::new();            
        if checkpoint.total_stakers == 0 || remaining_accounts.is_empty() {
            // If total stakers is zero, use default stake account
            ixs.push(ore_boost_api::sdk::rebase(
                self.signer().pubkey(),
                mint_address,
                Pubkey::default(),
            ));
            let _ = self.send_and_confirm(&ixs, ComputeBudget::Fixed(100_000), false)
                .await?;
        } else {
            // Chunk stake accounts into batches
            let chunks = remaining_accounts.chunks(MAX_ACCOUNTS_PER_TX);
            for chunk in chunks {
                ixs.clear();
                for (stake_pubkey, _stake) in chunk {
                    ixs.push(ore_boost_api::sdk::rebase(
                        self.signer().pubkey(),
                        mint_address,
                        *stake_pubkey,
                    ));
                }
                if !ixs.is_empty() {
                    let _ = self.send_and_confirm(&ixs, ComputeBudget::Fixed(100_000), false)
                        .await?;
                }
            }
        }

        progress_bar.finish_with_message(format!(
            "{} Checkpoint complete",
            "SUCCESS".green().bold()
        ));

        Ok(())
    }
} 

