use std::str::FromStr;
use solana_client::{rpc_filter::{self, Memcmp, RpcFilterType}, rpc_config::RpcProgramAccountsConfig};
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use ore_boost_api::state::{boost_pda, Boost, Stake, checkpoint_pda, Checkpoint};
use solana_rpc_client::spinner;
use colored::*;
use steel::{AccountDeserialize, Discriminator};

use crate::{
    args::CheckpointArgs,
    error::Error,
    send_and_confirm::ComputeBudget,
    Miner, utils::get_clock,
};

const MAX_ACCOUNTS_PER_TX: usize = 10;
const CHECKPOINT_INTERVAL: i64 = 24 * 60 * 60; // 24 hours in seconds

impl Miner {
    pub async fn checkpoint(&self, args: CheckpointArgs) -> Result<(), Error> {
        let progress_bar = spinner::new_progress_bar();
        progress_bar.set_message("Starting checkpoint process...");

        // Parse mint address
        let mint_address = Pubkey::from_str(&args.mint)?;
        let boost_address = boost_pda(mint_address).0;
        let checkpoint_address = checkpoint_pda(boost_address).0;

        // Get boost account data
        let boost_data = self.rpc_client.get_account_data(&boost_address).await?;
        let _boost = Boost::try_from_bytes(&boost_data)?;

        // Get checkpoint account data
        let checkpoint_data = self.rpc_client.get_account_data(&checkpoint_address).await?;
        let checkpoint = Checkpoint::try_from_bytes(&checkpoint_data)?;

        // Check if enough time has passed since last checkpoint
        let clock = get_clock(&self.rpc_client).await;
        let time_since_last = clock.unix_timestamp - checkpoint.ts;
        if time_since_last < CHECKPOINT_INTERVAL {
            progress_bar.finish_with_message(format!(
                "{} Not enough time has passed since last checkpoint. Wait {} more seconds.",
                "NOTICE".yellow(),
                CHECKPOINT_INTERVAL - time_since_last
            ));
            return Ok(());
        }

        // Get all stake accounts for this boost
        progress_bar.set_message("Fetching stake accounts...");
        let filters = vec![
            RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
                0,
                Stake::discriminator().to_le_bytes().to_vec(),
            )),
            RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
                48,
                boost_address.to_bytes().to_vec(),
            )),
        ];

        // Parse stake accounts
        let mut accounts: Vec<_> = self.rpc_client
            .get_program_accounts_with_config(
                &ore_boost_api::ID,
                RpcProgramAccountsConfig {
                    filters: Some(filters),
                    ..Default::default()
                },
            )
            .await?
            .into_iter()
            .map(|(pubkey, account)| {
                let stake = Stake::try_from_bytes(&account.data).unwrap().clone();
                (pubkey, stake)
            })
            .collect();

        println!("Stake accounts: {:?}", accounts);

        if accounts.is_empty() {
            progress_bar.finish_with_message("No stake accounts found for this boost");
            return Ok(());
        }

        // Sort accounts by stake ID
        accounts.sort_by(|(_, stake_a), (_, stake_b)| {
            stake_a.id.cmp(&stake_b.id)
        });

        progress_bar.set_message(format!("Processing stake accounts starting from ID {}...", checkpoint.current_id));

        // Filter accounts starting from checkpoint.current_id
        let remaining_accounts: Vec<_> = accounts
            .into_iter()
            .filter(|(_, stake)| {
                stake.id >= checkpoint.current_id && stake.id < checkpoint.total_stakers
            })
            .collect();


        let chunks = remaining_accounts.chunks(MAX_ACCOUNTS_PER_TX);
        println!("Chunks: {:?}", chunks);

        for chunk in chunks {
            let mut ixs = Vec::new();            
            println!("Chunk: {:?}", chunk);

            for (stake_pubkey, _stake) in chunk {
                // Only include active stakes
                println!("Stake pubkey: {:?}", stake_pubkey);
                ixs.push(ore_boost_api::sdk::rebase(
                    self.signer().pubkey(),
                    mint_address,
                    *stake_pubkey,
                ));
            }

            if !ixs.is_empty() {
                // Send transaction with batch of rebase instructions
                let sig = self.send_and_confirm(&ixs, ComputeBudget::Fixed(100_000), false)
                    .await?;
                println!("Rebase transaction: {}", sig);
            }
        }

        progress_bar.finish_with_message(format!(
            "{} Checkpoint completed successfully",
            "SUCCESS".green()
        ));

        Ok(())
    }
} 