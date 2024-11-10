use std::str::FromStr;

use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use steel::AccountDeserialize;

use crate::{
    args::{ BalanceArgs, BalanceCommand },
    error::Error,
    pool::Pool,
    utils::{ self, amount_u64_to_string, get_proof_with_authority },
    Miner,
};

impl Miner {
    pub async fn balance(&self, args: BalanceArgs) {
        if let Some(subcommand) = args.command.clone() {
            match subcommand {
                BalanceCommand::Commit(_) => self.balance_commit(args).await.unwrap(),
            }
        } else {
            match args.pool_url {
                None => {
                    self.balance_solo(&args).await;
                }
                Some(ref pool_url) => {
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

                    if let Err(err) = self.balance_pool(&address, pool_url).await {
                        println!("{:?}", err);
                    }
                }
            }
        }
    }

    async fn balance_commit(&self, args: BalanceArgs) -> Result<(), Error> {
        let Some(pool_url) = args.pool_url else {
            println!("Pool URL required");
            return Ok(());
        };
        let pool = Pool {
            http_client: reqwest::Client::new(),
            pool_url,
        };
        if let Err(err) = pool.post_update_balance(self).await {
            println!("{:?}", err);
        }
        Ok(())
    }

    async fn balance_pool(&self, address: &Pubkey, pool_url: &String) -> Result<(), Error> {
        // build pool client
        let pool = Pool {
            http_client: reqwest::Client::new(),
            pool_url: pool_url.clone(),
        };
        // Fetch token balance
        let token_account_address = spl_associated_token_account::get_associated_token_address(
            address,
            &ore_api::consts::MINT_ADDRESS
        );
        let token_balance = if
            let Ok(Some(token_account)) = self.rpc_client.get_token_account(
                &token_account_address
            ).await
        {
            token_account.token_amount.ui_amount_string
        } else {
            "0".to_string()
        };
        // fetch pool address
        let pool_address = pool.get_pool_address().await?;
        println!("Pool: {}", pool_address.address);
        // fetch on-chain balance
        let (member_pda, _) = ore_pool_api::state::member_pda(
            address.clone(),
            pool_address.address
        );
        let member_data = self.rpc_client.get_account_data(&member_pda).await?;
        let member = ore_pool_api::state::Member::try_from_bytes(member_data.as_slice())?;
        println!(
            "Balance: {} ORE\nPool yield: {} ORE",
            token_balance,
            utils::amount_u64_to_string(member.balance)
        );
        // fetch db balance
        let member_db = pool.get_pool_member(&self).await?;
        let diff = (member_db.total_balance as u64) - member.total_balance;
        println!("Pool yield (pending): {} ORE\n", utils::amount_u64_to_string(diff));
        println!("Pool operators automatically commit pending balances to the blockchain at regular intervals. To manually commit your pending balance now, run the following command:\n\n`ore balance --pool-url {} commit`\n", pool_url);
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
            &ore_api::consts::MINT_ADDRESS
        );
        let token_balance = if
            let Ok(Some(token_account)) = self.rpc_client.get_token_account(
                &token_account_address
            ).await
        {
            token_account.token_amount.ui_amount_string
        } else {
            "0".to_string()
        };
        println!(
            "Balance: {} ORE\nYield: {} ORE",
            token_balance,
            amount_u64_to_string(proof.balance)
        )
    }
}
