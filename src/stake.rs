use std::str::FromStr;

use colored::*;
use ore_boost_api::state::{boost_pda, stake_pda, Boost, Stake, Checkpoint, checkpoint_pda};
use solana_program::{program_pack::Pack, pubkey::Pubkey};
use solana_sdk::signature::Signer;
use spl_token::{amount_to_ui_amount, state::Mint};
use steel::AccountDeserialize;

use crate::{
    args::{StakeArgs, StakeCommand, StakeDepositArgs, StakeWithdrawArgs, StakeClaimArgs},
    cu_limits::CU_LIMIT_CLAIM,
    error::Error,
    send_and_confirm::ComputeBudget,
    Miner,
};

impl Miner {
    pub async fn stake(&self, args: StakeArgs) {
        if let Some(subcommand) = args.command.clone() {
            match subcommand {
                StakeCommand::Claim(subargs) => self.stake_claim(subargs, args).await.unwrap(),
                StakeCommand::Deposit(subargs) => self.stake_deposit(subargs, args).await.unwrap(),
                StakeCommand::Withdraw(subargs) => {
                    self.stake_withdraw(subargs, args).await.unwrap()
                }
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
        let stake_data = self.rpc_client.get_account_data(&stake_address).await?;
        let stake = Stake::try_from_bytes(&stake_data)?;

        // Build claim instruction with amount or max rewards
        ixs.push(ore_boost_api::sdk::claim(
            pubkey,
            stake_address, 
            beneficiary,
            claim_args.amount
                .map(|a| crate::utils::amount_f64_to_u64(a))
                .unwrap_or(stake.rewards),
        ));

        // Send and confirm transaction
        self.send_and_confirm(&ixs, ComputeBudget::Fixed(CU_LIMIT_CLAIM), false)
            .await?;

        Ok(())
    }

    async fn stake_get(&self, args: StakeArgs) -> Result<(), Error> {
        let mint_address = Pubkey::from_str(&args.mint).unwrap();
        let boost_address = boost_pda(mint_address).0;
        let checkpoint_address = checkpoint_pda(boost_address).0;
        let stake_address = stake_pda(self.signer().pubkey(), boost_address).0;
        let Ok(boost_data) = self.rpc_client.get_account_data(&boost_address).await else {
            println!("No boost found for mint: {}", mint_address);
            return Ok(());
        };
        let Ok(boost) = Boost::try_from_bytes(&boost_data) else {
            println!("Failed to parse boost data");
            return Ok(());
        };
        let Ok(checkpoint_data) = self.rpc_client.get_account_data(&checkpoint_address).await else {
            println!("Failed to fetch checkpoint data");
            return Ok(());
        };
        let Ok(checkpoint) = Checkpoint::try_from_bytes(&checkpoint_data) else {
            println!("Failed to parse checkpoint data");
            return Ok(());
        };
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
                println!("Yield: {}", stake.rewards);
            }
        };
        println!("\n{}", "Boost".bold());
        println!("Mint: {}", mint_address);
        println!(
            "Balance: {} {}",
            amount_to_ui_amount(boost.total_stake, mint.decimals),
            symbol
        );
        println!("Multiplier: {}x", boost.multiplier);
        println!("Miner: {}", boost.proof);
        println!("Expires at: {}", boost.expires_at);
        println!("Reserved at: {}", boost.reserved_at);
        println!("\n{}", "Checkpoint".bold());
        println!("Current: {}", checkpoint.current_id);
        println!("Total stakers: {}", checkpoint.total_stakers);
        println!("Total rewards: {}", amount_to_ui_amount(checkpoint.total_rewards, mint.decimals));
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

        // Fetch boost
        let Ok(boost_account_data) = self.rpc_client.get_account_data(&boost_address).await else {
            println!("Failed to fetch boost account");
            return Ok(());
        };
        let _ = Boost::try_from_bytes(&boost_account_data).unwrap();

        // Open stake account, if needed
        if let Err(_err) = self.rpc_client.get_account_data(&stake_address).await {
            println!("Failed to fetch stake account");
            let ix = ore_boost_api::sdk::open(signer.pubkey(), signer.pubkey(), mint_address);
            self.send_and_confirm(&[ix], ComputeBudget::Fixed(CU_LIMIT_CLAIM), false)
                .await
                .ok();
        }

        // Send tx
        let ix = ore_boost_api::sdk::deposit(signer.pubkey(), mint_address, amount);
        self.send_and_confirm(&[ix], ComputeBudget::Fixed(CU_LIMIT_CLAIM), false)
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

        // Fetch boost
        let Ok(boost_account_data) = self.rpc_client.get_account_data(&boost_address).await else {
            println!("Failed to fetch boost account");
            return Ok(());
        };
        let _ = Boost::try_from_bytes(&boost_account_data).unwrap();

        // Fetch stake account, if needed
        let Ok(stake_account_data) = self.rpc_client.get_account_data(&stake_address).await else {
            println!("Failed to fetch stake account");
            return Ok(());
        };
        let stake = Stake::try_from_bytes(&stake_account_data).unwrap();

        // Parse amount
        let amount: u64 = if let Some(amount) = args.amount {
            (amount * 10f64.powf(mint.decimals as f64)) as u64
        } else {
            stake.balance
        };

        // Send tx
        // TODO: benfeciary should be arg to ix builder
        let ix = ore_boost_api::sdk::withdraw(signer.pubkey(), mint_address, amount);
        self.send_and_confirm(&[ix], ComputeBudget::Fixed(CU_LIMIT_CLAIM), false)
            .await
            .ok();

        Ok(())
    }
}
