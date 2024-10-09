use std::str::FromStr;

use ore_boost_api::state::{boost_pda, stake_pda, Boost, Stake};
use solana_program::{program_pack::Pack, pubkey::Pubkey};
use solana_sdk::signature::Signer;
use spl_token::state::Mint;
use steel::AccountDeserialize;

use crate::{
    args::StakeArgs, cu_limits::CU_LIMIT_CLAIM, error::Error, pool::Pool,
    send_and_confirm::ComputeBudget, Miner,
};

impl Miner {
    pub async fn stake(&self, args: StakeArgs) {
        match args.pool_url {
            None => {
                self.stake_solo(&args).await;
            }
            Some(ref pool_url) => {
                if let Err(err) = self.stake_pool(&args, pool_url).await {
                    println!("{:?}", err);
                }
            }
        }
    }
    async fn stake_pool(&self, args: &StakeArgs, pool_url: &String) -> Result<(), Error> {
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
        let mint = Pubkey::from_str(args.mint.as_str())?;
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
    async fn stake_solo(&self, args: &StakeArgs) {
        // Parse mint address
        let mint_address = Pubkey::from_str(&args.mint).unwrap();

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
            return;
        };

        let Ok(mint_data) = self.rpc_client.get_account_data(&mint_address).await else {
            println!("Failed to fetch mint address");
            return;
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
            return;
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
    }
}
