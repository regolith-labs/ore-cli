use std::str::FromStr;

use colored::*;
use ore_api::state::proof_pda;
use ore_boost_api::{state::{boost_pda, stake_pda, Stake, checkpoint_pda}, consts::BOOST_DENOMINATOR};
use solana_program::{program_pack::Pack, pubkey::Pubkey};
use solana_sdk::signature::Signer;
use spl_token::{amount_to_ui_amount, state::Mint};
use steel::AccountDeserialize;

use crate::{
    args::{StakeArgs, StakeCommand, StakeDepositArgs, StakeWithdrawArgs, StakeClaimArgs, StakeMigrateArgs},
    error::Error,
    send_and_confirm::ComputeBudget,
    Miner, utils::{get_boost, get_checkpoint, get_stake, get_proof, get_legacy_stake}, pool::Pool,
};

impl Miner {
    pub async fn stake(&self, args: StakeArgs) {
        if let Some(subcommand) = args.command.clone() {
            match subcommand {
                StakeCommand::Claim(subargs) => self.stake_claim(subargs, args).await.unwrap(),
                StakeCommand::Deposit(subargs) => self.stake_deposit(subargs, args).await.unwrap(),
                StakeCommand::Withdraw(subargs) => self.stake_withdraw(subargs, args).await.unwrap(),
                StakeCommand::Migrate(subargs) => self.stake_migrate(subargs, args).await.unwrap(),
            }
        } else {
            self.stake_get(args).await.unwrap();
        }
    }

    async fn stake_claim(&self, claim_args: StakeClaimArgs, stake_args: StakeArgs) -> Result<(), Error> {
        let signer = self.signer();
        let pubkey = signer.pubkey();
        let mint_address = Pubkey::from_str(&stake_args.mint).unwrap();
        let boost_address = boost_pda(mint_address).0;
        let stake_address = stake_pda(pubkey, boost_address).0;

        let mut ixs = vec![];
        let beneficiary = match claim_args.to {
            None => self.initialize_ata(pubkey).await,
            Some(to) => {
                let wallet = Pubkey::from_str(&to).expect("Failed to parse wallet address");
                let beneficiary_tokens = spl_associated_token_account::get_associated_token_address(
                    &wallet,
                    &ore_api::consts::MINT_ADDRESS,
                );
                if self.rpc_client.get_token_account(&beneficiary_tokens).await.is_err() {
                    ixs.push(
                        spl_associated_token_account::instruction::create_associated_token_account(
                            &pubkey,
                            &wallet,
                            &ore_api::consts::MINT_ADDRESS,
                            &spl_token::id(),
                        ),
                    );
                }
                beneficiary_tokens
            }
        };

        // Get stake account data to check rewards balance
        let stake = get_stake(&self.rpc_client, stake_address).await;

        // Build claim instruction with amount or max rewards
        ixs.push(ore_boost_api::sdk::claim(
            pubkey,
            beneficiary,
            mint_address,
            claim_args.amount
                .map(|a| crate::utils::amount_f64_to_u64(a))
                .unwrap_or(stake.rewards),
        ));

        // Send and confirm transaction
        self.send_and_confirm(&ixs, ComputeBudget::Fixed(32_000), false)
            .await?;

        Ok(())
    }

    async fn stake_get(&self, args: StakeArgs) -> Result<(), Error> {
        let mint_address = Pubkey::from_str(&args.mint).unwrap();
        let boost_address = boost_pda(mint_address).0;
        let checkpoint_address = checkpoint_pda(boost_address).0;
        let stake_address = stake_pda(self.signer().pubkey(), boost_address).0;
        let boost_proof_address = proof_pda(boost_address).0;
        let boost = get_boost(&self.rpc_client, boost_address).await;
        let boost_proof = get_proof(&self.rpc_client, boost_proof_address).await;
        let checkpoint = get_checkpoint(&self.rpc_client, checkpoint_address).await;
        let Ok(mint_data) = self.rpc_client.get_account_data(&mint_address).await else {
            println!("Failed to fetch mint data");
            return Ok(());
        };
        let Ok(mint) = Mint::unpack(&mint_data) else {
            println!("Failed to parse mint data");
            return Ok(());
        };
        let metadata_address = mpl_token_metadata::accounts::Metadata::find_pda(&mint_address).0;
        let symbol = match self.rpc_client.get_account_data(&metadata_address).await {
            Ok(metadata_data) => {
                if let Ok(metadata) = mpl_token_metadata::accounts::Metadata::from_bytes(&metadata_data) {
                    metadata.symbol
                } else {
                    "".to_string()
                }
            }
            Err(_) => "".to_string()
        };
        if let Ok(stake_data) = self.rpc_client.get_account_data(&stake_address).await {
            if let Ok(stake) = Stake::try_from_bytes(&stake_data) {
                println!("{}", "Stake".bold());
                println!("Address: {}", stake_address);
                println!(
                    "Balance: {} {} ({:.8}% of total)",
                    amount_to_ui_amount(stake.balance, mint.decimals),
                    symbol,
                    (stake.balance as f64 / boost.total_stake as f64) * 100f64
                );
                println!(
                    "Balance (pending): {} {}",
                    amount_to_ui_amount(stake.pending_balance, mint.decimals),
                    symbol,
                );
                println!("Last deposit at: {}", stake.last_deposit_at);
                println!("Yield: {} ORE", amount_to_ui_amount(stake.rewards, ore_api::consts::TOKEN_DECIMALS));
            }
        };
        println!("\n{}", "Boost".bold());
        println!("Mint: {}", mint_address);
        println!(
            "Deposits: {} {}",
            amount_to_ui_amount(boost.total_stake, mint.decimals),
            symbol
        );
        println!("Yield (pending): {} ORE", amount_to_ui_amount(boost_proof.balance, ore_api::consts::TOKEN_DECIMALS));
        println!("Multiplier: {}x", boost.multiplier as f64 / BOOST_DENOMINATOR as f64);
        println!("Expires at: {}", boost.expires_at);
        println!("Locked: {}", boost.locked);
        println!("\n{}", "Checkpoint".bold());
        println!("Current: {}", checkpoint.current_id);
        println!("Total stakers: {}", checkpoint.total_stakers);
        println!("Total rewards: {} ORE", amount_to_ui_amount(checkpoint.total_rewards, ore_api::consts::TOKEN_DECIMALS));
        println!("Timestamp: {}", checkpoint.ts);
        Ok(())
    }

    async fn stake_deposit(
        &self,
        args: StakeDepositArgs,
        stake_args: StakeArgs,
    ) -> Result<(), Error> {
        // Parse mint address
        let mint_address = Pubkey::from_str(&stake_args.mint).unwrap();

        // Get signer
        let signer = self.signer();
        let sender = match &args.token_account {
            Some(address) => {
                Pubkey::from_str(&address).expect("Failed to parse token account address")
            }
            None => spl_associated_token_account::get_associated_token_address(
                &signer.pubkey(),
                &mint_address,
            ),
        };

        // Get token account
        let Ok(Some(token_account)) = self.rpc_client.get_token_account(&sender).await else {
            println!("Failed to fetch token account");
            return Ok(());
        };

        let Ok(mint_data) = self.rpc_client.get_account_data(&mint_address).await else {
            println!("Failed to fetch mint address");
            return Ok(());
        };
        let mint = Mint::unpack(&mint_data).unwrap();

        // Parse amount
        let amount: u64 = if let Some(amount) = args.amount {
            (amount * 10f64.powf(mint.decimals as f64)) as u64
        } else {
            u64::from_str(token_account.token_amount.amount.as_str())
                .expect("Failed to parse token balance")
        };

        // Get addresses
        let boost_address = boost_pda(mint_address).0;
        let stake_address = stake_pda(signer.pubkey(), boost_address).0;
        let _boost = get_boost(&self.rpc_client, boost_address).await;

        // Open stake account, if needed
        if let Err(_err) = self.rpc_client.get_account_data(&stake_address).await {
            println!("Failed to fetch stake account");
            let ix = ore_boost_api::sdk::open(signer.pubkey(), signer.pubkey(), mint_address);
            self.send_and_confirm(&[ix], ComputeBudget::Fixed(32_000), false)
                .await
                .ok();
        }

        // Send tx
        let ix = ore_boost_api::sdk::deposit(signer.pubkey(), mint_address, amount);
        self.send_and_confirm(&[ix], ComputeBudget::Fixed(32_000), false)
            .await
            .ok();

        Ok(())
    }

    async fn stake_withdraw(
        &self,
        args: StakeWithdrawArgs,
        stake_args: StakeArgs,
    ) -> Result<(), Error> {
        // Parse mint address
        let mint_address = Pubkey::from_str(&stake_args.mint).unwrap();

        // Get signer
        let signer = self.signer();
        let beneficiary = match &args.token_account {
            Some(address) => {
                Pubkey::from_str(&address).expect("Failed to parse token account address")
            }
            None => spl_associated_token_account::get_associated_token_address(
                &signer.pubkey(),
                &mint_address,
            ),
        };

        // Get token account
        let Ok(Some(_token_account)) = self.rpc_client.get_token_account(&beneficiary).await else {
            println!("Failed to fetch token account");
            return Ok(());
        };

        let Ok(mint_data) = self.rpc_client.get_account_data(&mint_address).await else {
            println!("Failed to fetch mint address");
            return Ok(());
        };
        let mint = Mint::unpack(&mint_data).unwrap();

        // Get addresses
        let boost_address = boost_pda(mint_address).0;
        let stake_address = stake_pda(signer.pubkey(), boost_address).0;
        let _boost = get_boost(&self.rpc_client, boost_address).await;
        let stake = get_stake(&self.rpc_client, stake_address).await;
        
        // Parse amount
        let amount: u64 = if let Some(amount) = args.amount {
            (amount * 10f64.powf(mint.decimals as f64)) as u64
        } else {
            stake.balance
        };

        // Send tx
        // TODO: benfeciary should be arg to ix builder
        let ix = ore_boost_api::sdk::withdraw(signer.pubkey(), mint_address, amount);
        self.send_and_confirm(&[ix], ComputeBudget::Fixed(32_000), false)
            .await
            .ok();

        Ok(())
    }

    async fn stake_migrate(&self, args: StakeMigrateArgs, stake_args: StakeArgs) -> Result<(), Error> {
        if args.pool_url.is_some() {
            self.stake_migrate_pool(args, stake_args).await
        } else {
            self.stake_migrate_solo(args, stake_args).await
        }
    }

    async fn stake_migrate_solo(&self, _args: StakeMigrateArgs, stake_args: StakeArgs) -> Result<(), Error> {
        let signer = self.signer();
        let pubkey = signer.pubkey();
        let mint_address = Pubkey::from_str(&stake_args.mint).unwrap();
        let legacy_boost_address = ore_boost_legacy_api::state::boost_pda(mint_address).0;
        let legacy_stake_address = ore_boost_legacy_api::state::stake_pda(pubkey, legacy_boost_address).0;
        let boost_address = boost_pda(mint_address).0;
        let stake_address = stake_pda(pubkey, boost_address).0;
        let mut ixs = vec![];

        // Withdraw from legacy boost
        let legacy_stake_account = get_legacy_stake(&self.rpc_client, legacy_stake_address).await;
        ixs.push(
            ore_boost_legacy_api::sdk::withdraw(
                pubkey, 
                mint_address, 
                legacy_stake_account.balance
            )
        );

        // Open new stake account
        if self.rpc_client.get_account_data(&stake_address).await.is_err() {
            ixs.push(ore_boost_api::sdk::open(pubkey, pubkey, mint_address));
        }

        // Deposit into new stake account
        ixs.push(ore_boost_api::sdk::deposit(pubkey, mint_address, legacy_stake_account.balance));

        // Send and confirm transaction
        self.send_and_confirm(&ixs, ComputeBudget::Fixed(50_000), false)
            .await
            .ok();

        Ok(())
    }

    async fn stake_migrate_pool(&self, args: StakeMigrateArgs, stake_args: StakeArgs) -> Result<(), Error> {
        let signer = self.signer();
        let pubkey = signer.pubkey();
        let mint_address = Pubkey::from_str(&stake_args.mint).unwrap();
        let boost_address = boost_pda(mint_address).0;
        let stake_address = stake_pda(pubkey, boost_address).0;
        let mut ixs = vec![];

        // Get pool address
        let pool = Pool {
            http_client: reqwest::Client::new(),
            pool_url: args.pool_url.unwrap(),
        };
        let Ok(pool_address) = pool.get_pool_address().await else {
            println!("Pool not found");
            return Ok(());
        };

        // Get pool share account
        let share = pool
            .get_staker_onchain(self, pool_address.address, mint_address)
            .await?;
        
        // Withdraw from share account
        let recipient = spl_associated_token_account::get_associated_token_address(
            &pubkey,
            &mint_address,
        );
        ixs.push(ore_pool_api::sdk::unstake(pubkey, mint_address, pool_address.address, recipient, share.balance));

        // Open new stake account
        if self.rpc_client.get_account_data(&stake_address).await.is_err() {
            ixs.push(ore_boost_api::sdk::open(pubkey, pubkey, mint_address));
        }

        // Deposit into new stake account
        ixs.push(ore_boost_api::sdk::deposit(pubkey, mint_address, share.balance));

        // Send and confirm transaction
        self.send_and_confirm(&ixs, ComputeBudget::Fixed(500_000), false)
            .await
            .ok();

        Ok(())
    }
}
