use std::str::FromStr;

use ore_boost_api::state::{boost_pda, stake_pda, Boost, Stake};
use solana_program::{program_pack::Pack, pubkey::Pubkey};
use solana_sdk::{signature::Signer, transaction::Transaction};
use spl_token::state::Mint;
use steel::AccountDeserialize;

use crate::{
    args::UnstakeArgs, cu_limits::CU_LIMIT_CLAIM, error::Error, pool::Pool,
    send_and_confirm::ComputeBudget, Miner,
};

impl Miner {
    pub async fn unstake(&self, args: UnstakeArgs) {
        match args.pool_url {
            None => {
                self.unstake_solo(&args).await;
            }
            Some(ref pool_url) => {
                if let Err(err) = self.unstake_pool(&args, pool_url).await {
                    println!("{:?}", err);
                }
            }
        }
    }
    async fn unstake_pool(&self, args: &UnstakeArgs, pool_url: &String) -> Result<(), Error> {
        let signer = self.signer();
        // build pool client
        let pool = Pool {
            http_client: reqwest::Client::new(),
            pool_url: pool_url.clone(),
        };
        // parse mint
        let mint_address = Pubkey::from_str(args.mint.as_str())?;
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
    async fn unstake_solo(&self, args: &UnstakeArgs) {
        // Parse mint address
        let mint_address = Pubkey::from_str(&args.mint).unwrap();

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
            return;
        };

        let Ok(mint_data) = self.rpc_client.get_account_data(&mint_address).await else {
            println!("Failed to fetch mint address");
            return;
        };
        let mint = Mint::unpack(&mint_data).unwrap();

        // Get addresses
        let boost_address = boost_pda(mint_address).0;
        let stake_address = stake_pda(signer.pubkey(), boost_address).0;

        // Fetch boost
        let Ok(boost_account_data) = self.rpc_client.get_account_data(&boost_address).await else {
            println!("Failed to fetch boost account");
            return;
        };
        let _ = Boost::try_from_bytes(&boost_account_data).unwrap();

        // Fetch stake account, if needed
        let Ok(stake_account_data) = self.rpc_client.get_account_data(&stake_address).await else {
            println!("Failed to fetch stake account");
            return;
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
    }
}
