use std::str::FromStr;

use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use steel::AccountDeserialize;

use crate::{
    args::BalanceArgs,
    error::Error,
    pool::Pool,
    utils::{self, amount_u64_to_string, get_proof_with_authority},
    Miner,
};

impl Miner {
    pub async fn balance(&self, args: BalanceArgs) {
        match args.pool_url {
            None => {
                self.balance_solo(&args).await;
            }
            Some(ref pool_url) => {
                if let Err(err) = self.balance_pool(pool_url).await {
                    println!("{:?}", err);
                }
            }
        }
    }
    async fn balance_pool(&self, pool_url: &String) -> Result<(), Error> {
        let signer = self.signer();
        // build pool client
        let pool = Pool {
            http_client: reqwest::Client::new(),
            pool_url: pool_url.clone(),
        };
        // fetch pool address
        let pool_address = pool.get_pool_address().await?;
        // fetch on-chain balance
        let (member_pda, _) =
            ore_pool_api::state::member_pda(signer.pubkey(), pool_address.address);
        let member_data = self.rpc_client.get_account_data(&member_pda).await?;
        let member = ore_pool_api::state::Member::try_from_bytes(member_data.as_slice())?;
        println!("//////////////////////////////");
        println!(
            "your on-chain pool balance: {:?}",
            utils::amount_u64_to_string(member.balance)
        );
        // fetch db balance
        let member_db = pool.get_pool_member(&self).await?;
        let diff = (member_db.total_balance as u64) - member.total_balance;
        if diff.gt(&0) {
            println!("//////////////////////////////");
            println!(
                "you have an amount pending on-chain attribution: {:?}",
                utils::amount_u64_to_string(diff)
            );
            println!("the pool operator automatically attributes your on-chain balance at regular intervals.");
            println!("if you want to attribute this balance yourself now, you can pay the transaction fee by running the 'ore update-pool-balance {}' command.", pool_url);
            println!("for more info run the 'ore help' command.");
        }
        Ok(())
    }
    async fn balance_solo(&self, args: &BalanceArgs) {
        let signer = self.signer();
        let address = if let Some(address) = &args.address {
            if let Ok(address) = Pubkey::from_str(&address) {
                address
            } else {
                println!("Invalid address: {:?}", address);
                return;
            }
        } else {
            signer.pubkey()
        };
        let proof = get_proof_with_authority(&self.rpc_client, address).await;
        let token_account_address = spl_associated_token_account::get_associated_token_address(
            &address,
            &ore_api::consts::MINT_ADDRESS,
        );
        let token_balance = if let Ok(Some(token_account)) = self
            .rpc_client
            .get_token_account(&token_account_address)
            .await
        {
            token_account.token_amount.ui_amount_string
        } else {
            "0".to_string()
        };
        println!(
            "Balance: {} ORE\nStake: {} ORE",
            token_balance,
            amount_u64_to_string(proof.balance)
        )
    }
}
