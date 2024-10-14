use std::str::FromStr;

use colored::*;
use ore_boost_api::state::{boost_pda, stake_pda, Boost, Stake};
use ore_pool_api::state::{share_pda, Share};
use solana_program::{program_pack::Pack, pubkey::Pubkey};
use solana_sdk::{signature::Signer, transaction::Transaction};
use spl_token::state::Mint;
use steel::AccountDeserialize;

use crate::{
    args::{StakeArgs, StakeCommand, StakeDepositArgs, StakeWithdrawArgs},
    cu_limits::CU_LIMIT_CLAIM,
    error::Error,
    pool::Pool,
    send_and_confirm::ComputeBudget,
    Miner,
};

impl Miner {
    pub async fn stake(&self, args: StakeArgs) {
        match args.command.clone() {
            StakeCommand::Get(_) => self.stake_get(args).await.unwrap(),
            StakeCommand::Deposit(subargs) => self.stake_deposit(subargs, args).await.unwrap(),
            StakeCommand::Withdraw(subargs) => self.stake_withdraw(subargs, args).await.unwrap(),
        }
    }

    async fn stake_get(&self, args: StakeArgs) -> Result<(), Error> {
        match args.pool_url.clone() {
            None => self.stake_get_solo(args).await,
            Some(ref pool_url) => self.stake_get_pool(args, pool_url).await,
        }
    }

    async fn stake_get_solo(&self, args: StakeArgs) -> Result<(), Error> {
        let mint_address = Pubkey::from_str(&args.mint).unwrap();
        let boost_address = boost_pda(mint_address).0;
        let stake_address = stake_pda(self.signer().pubkey(), boost_address).0;
        let Ok(boost_data) = self.rpc_client.get_account_data(&boost_address).await else {
            return Ok(());
        };
        let Ok(boost) = Boost::try_from_bytes(&boost_data) else {
            return Ok(());
        };
        let Ok(stake_data) = self.rpc_client.get_account_data(&stake_address).await else {
            return Ok(());
        };
        let Ok(stake) = Stake::try_from_bytes(&stake_data) else {
            return Ok(());
        };
        println!("{}", "Stake".bold());
        println!("Address: {}", stake_address);
        println!(
            "Balance: {} ({:.8}% of total)",
            stake.balance,
            (stake.balance as f64 / boost.total_stake as f64) * 100f64
        );
        println!("Last stake at: {}", stake.last_stake_at);
        println!("\n{}", "Boost".bold());
        println!("Balance: {}", boost.total_stake);
        println!("Mint: {}", mint_address);
        println!("Balance: {}", boost.total_stake);
        println!("Multiplier: {}x", boost.multiplier);
        println!("Expires at: {}", boost.expires_at);
        Ok(())
    }

    async fn stake_get_pool(&self, args: StakeArgs, pool_url: &String) -> Result<(), Error> {
        let pool = Pool {
            http_client: reqwest::Client::new(),
            pool_url: pool_url.clone(),
        };
        let pool_address = pool.get_pool_address().await?.address;
        let mint_address = Pubkey::from_str(&args.mint).unwrap();
        let boost_address = boost_pda(mint_address).0;
        let stake_address = stake_pda(pool_address, boost_address).0;
        let share_address = share_pda(self.signer().pubkey(), pool_address, mint_address).0;
        let Ok(boost_data) = self.rpc_client.get_account_data(&boost_address).await else {
            return Ok(());
        };
        let Ok(boost) = Boost::try_from_bytes(&boost_data) else {
            return Ok(());
        };
        let Ok(stake_data) = self.rpc_client.get_account_data(&stake_address).await else {
            return Ok(());
        };
        let Ok(stake) = Stake::try_from_bytes(&stake_data) else {
            return Ok(());
        };
        let Ok(share_data) = self.rpc_client.get_account_data(&share_address).await else {
            return Ok(());
        };
        let Ok(share) = Share::try_from_bytes(&share_data) else {
            return Ok(());
        };
        println!("{}", "Share".bold());
        println!("Address: {}", share_address);
        println!(
            "Balance: {} ({:.8}% of pool)",
            share.balance,
            (share.balance as f64 / stake.balance as f64) * 100f64
        );
        println!("\n{}", "Pool".bold());
        println!("Address: {}", pool_address);
        println!(
            "Balance: {} ({:.8}% of total)",
            stake.balance,
            (stake.balance as f64 / boost.total_stake as f64) * 100f64
        );
        println!("URL: {}", pool_url);
        println!("Last stake at: {}", stake.last_stake_at);
        println!("\n{}", "Boost".bold());
        println!("Balance: {}", boost.total_stake);
        println!("Mint: {}", mint_address);
        println!("Multiplier: {}x", boost.multiplier);
        println!("Expires at: {}", boost.expires_at);
        Ok(())
    }

    async fn stake_deposit(
        &self,
        args: StakeDepositArgs,
        stake_args: StakeArgs,
    ) -> Result<(), Error> {
        match stake_args.pool_url.clone() {
            None => self.stake_deposit_solo(args, stake_args).await,
            Some(ref pool_url) => self.stake_deposit_pool(args, stake_args, pool_url).await,
        }
    }
    async fn stake_deposit_solo(
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

    async fn stake_deposit_pool(
        &self,
        args: StakeDepositArgs,
        stake_args: StakeArgs,
        pool_url: &String,
    ) -> Result<(), Error> {
        let signer = self.signer();
        // build pool client
        let pool = Pool {
            http_client: reqwest::Client::new(),
            pool_url: pool_url.clone(),
        };
        // register member, if needed
        let _ = pool.post_pool_register(self).await?;
        // fetch pool address
        let pool_address = pool.get_pool_address().await?;
        // parse mint
        let mint = Pubkey::from_str(stake_args.mint.as_str())?;
        // get sender token account
        let sender = match &args.token_account {
            Some(address) => Pubkey::from_str(address.as_str())?,
            None => {
                spl_associated_token_account::get_associated_token_address(&signer.pubkey(), &mint)
            }
        };
        // assert that sender exists
        let Ok(Some(token_account)) = self.rpc_client.get_token_account(&sender).await else {
            println!("Failed to fetch token account");
            return Err(Error::Internal(
                "sender token account does not exist".to_string(),
            ));
        };
        // assert that mint exists
        let mint_data = self.rpc_client.get_account_data(&mint).await?;
        let mint_account = Mint::unpack(&mint_data)?;
        // parse amount
        let amount: u64 = if let Some(amount) = args.amount {
            (amount * 10f64.powf(mint_account.decimals as f64)) as u64
        } else {
            u64::from_str(token_account.token_amount.amount.as_str())?
        };
        // derive pdas
        let boost_address = boost_pda(mint).0;
        let stake_address = stake_pda(pool_address.address, boost_address).0;
        // assert that boost exists
        let boost_data = self.rpc_client.get_account_data(&boost_address).await?;
        let _ = Boost::try_from_bytes(boost_data.as_slice())?;
        // assert that stake exists (belongs to pool account)
        let stake_data = self.rpc_client.get_account_data(&stake_address).await?;
        let _ = Stake::try_from_bytes(stake_data.as_slice())?;
        // open share account, if needed
        let _ = pool.post_pool_register_staker(self, &mint).await?;
        // send tx
        let ix =
            ore_pool_api::sdk::stake(signer.pubkey(), mint, pool_address.address, sender, amount);
        let _ = self
            .send_and_confirm(&[ix], ComputeBudget::Fixed(CU_LIMIT_CLAIM), false)
            .await?;
        Ok(())
    }

    async fn stake_withdraw(
        &self,
        args: StakeWithdrawArgs,
        stake_args: StakeArgs,
    ) -> Result<(), Error> {
        match stake_args.pool_url.clone() {
            None => self.stake_withdraw_solo(args, stake_args).await,
            Some(ref pool_url) => self.stake_withdraw_pool(args, stake_args, pool_url).await,
        }
    }

    async fn stake_withdraw_solo(
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

    async fn stake_withdraw_pool(
        &self,
        args: StakeWithdrawArgs,
        stake_args: StakeArgs,
        pool_url: &String,
    ) -> Result<(), Error> {
        let signer = self.signer();
        // build pool client
        let pool = Pool {
            http_client: reqwest::Client::new(),
            pool_url: pool_url.clone(),
        };
        // parse mint
        let mint_address = Pubkey::from_str(stake_args.mint.as_str())?;
        // get beneficiary
        let beneficiary = match &args.token_account {
            Some(address) => Pubkey::from_str(&address)?,
            None => spl_associated_token_account::get_associated_token_address(
                &signer.pubkey(),
                &mint_address,
            ),
        };
        // assert that token account exists
        let Ok(Some(_token_account)) = self.rpc_client.get_token_account(&beneficiary).await else {
            return Err(Error::Internal("failed to fetch token account".to_string()));
        };
        // fetch mint account
        let mint_data = self.rpc_client.get_account_data(&mint_address).await?;
        let mint = Mint::unpack(&mint_data)?;
        // assert that boost account exists
        let boost_address = boost_pda(mint_address).0;
        let boost_account_data = self.rpc_client.get_account_data(&boost_address).await?;
        let _boost = Boost::try_from_bytes(boost_account_data.as_slice())?;
        // fetch share account
        let pool_address = pool.get_pool_address().await?;
        let share = pool
            .get_staker_onchain(self, pool_address.address, mint_address)
            .await?;
        // parse amount
        let amount: u64 = if let Some(amount) = args.amount {
            (amount * 10f64.powf(mint.decimals as f64)) as u64
        } else {
            share.balance
        };
        // send tx
        let ix = ore_pool_api::sdk::unstake(
            signer.pubkey(),
            mint_address,
            pool_address.address,
            beneficiary,
            amount,
        );
        let mut tx = Transaction::new_with_payer(&[ix], Some(&signer.pubkey()));
        let hash = self.rpc_client.get_latest_blockhash().await?;
        tx.sign(&[&signer], hash);
        let sig = self.rpc_client.send_transaction(&tx).await?;
        println!("{:?}", sig);
        Ok(())
    }
}
