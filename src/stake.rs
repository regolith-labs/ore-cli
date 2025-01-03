use std::str::FromStr;

use colored::*;
use ore_api::state::proof_pda;
use ore_boost_api::{state::{boost_pda, stake_pda, Boost}, consts::BOOST_DENOMINATOR};
use solana_program::{program_pack::Pack, pubkey::Pubkey};
use solana_sdk::signature::Signer;
use spl_token::{amount_to_ui_amount, state::Mint};
use tabled::{Table, settings::{Style, Color, Remove, object::{Rows, Columns}, Alignment, Highlight, style::BorderColor, Border}, Tabled};

use crate::{
    args::{StakeArgs, StakeCommand, StakeDepositArgs, StakeWithdrawArgs, StakeClaimArgs, StakeMigrateArgs},
    error::Error,
    send_and_confirm::ComputeBudget,
    Miner, utils::{get_boost, get_stake, get_proof, get_legacy_stake, TableData, format_timestamp, amount_u64_to_f64, get_boosts, get_mint, TableSectionTitle}, pool::Pool,
};

#[derive(Tabled)]
pub struct StakeTableData {
    #[tabled(rename = "Mint")]
    pub mint: String,
    #[tabled(rename = "Multiplier")]
    pub multiplier: String,
    #[tabled(rename = "Stakers")]
    pub total_stakers: String,
    #[tabled(rename = "Deposits")]
    pub total_deposits: String,
    #[tabled(rename = "My deposits")]
    pub my_deposits: String,
    #[tabled(rename = "My share")]
    pub my_share: String,
    #[tabled(rename = "My yield")]
    pub my_yield: String,
}

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
            if let Some(mint) = args.mint {
                self.stake_get(mint).await.unwrap();
            } else {
                self.stake_list().await.unwrap();
            }
        }
    }

    async fn stake_claim(&self, claim_args: StakeClaimArgs, stake_args: StakeArgs) -> Result<(), Error> {
        let signer = self.signer();
        let pubkey = signer.pubkey();
        let mint_address = Pubkey::from_str(&stake_args.mint.unwrap()).unwrap();
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
        let stake = get_stake(&self.rpc_client, stake_address).await.unwrap();

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
        println!("Claiming staking yield...");
        match self.send_and_confirm(&ixs, ComputeBudget::Fixed(32_000), false).await {
            Ok(sig) => println!("{} {}", "OK".green().bold(), sig),
            Err(e) => println!("{}: {}", "ERROR".bold().red(), e),
        }

        Ok(())
    }

    async fn stake_get(&self, mint: String) -> Result<(), Error> {
        // Fetch onchain data
        let mint_address = Pubkey::from_str(&mint).unwrap();
        let boost_address = boost_pda(mint_address).0;
        let stake_address = stake_pda(self.signer().pubkey(), boost_address).0;
        let boost = get_boost(&self.rpc_client, boost_address).await.unwrap();
        let mint = get_mint(&self.rpc_client, mint_address).await.unwrap();
        let metadata_address = mpl_token_metadata::accounts::Metadata::find_pda(&mint_address).0;
        let symbol = match self.rpc_client.get_account_data(&metadata_address).await {
            Ok(metadata_data) => {
                if let Ok(metadata) = mpl_token_metadata::accounts::Metadata::from_bytes(&metadata_data) {
                    format!(" {} ", metadata.symbol)
                } else {
                    " ".to_string()
                }
            }
            Err(_) => " ".to_string()
        };

        // Aggregate data
        let mut data = vec![];
        self.fetch_boost_data(boost_address, boost, mint, symbol.clone(), &mut data).await;
        let len1 = data.len();
        self.fetch_stake_data(stake_address, boost, mint, symbol.clone(), &mut data).await;
        let _len2 = data.len();

        // Build table
        let mut table = Table::new(data);
        table.with(Remove::row(Rows::first()));
        table.modify(Columns::single(1), Alignment::right());
        table.with(Style::blank());
        table.section_title(0, "Boost");
        table.section_title(len1, "Stake");
        println!("{table}\n");
        Ok(())
    }

    async fn fetch_stake_data(&self, address: Pubkey, boost: Boost, mint: Mint, symbol: String, data: &mut Vec<TableData>) {
        let stake = get_stake(&self.rpc_client, address).await;
        data.push(TableData {
            key: "Address".to_string(),
            value: address.to_string(),
        });
        if let Ok(stake) = stake {
            data.push(TableData {
                key: "Deposits".to_string(),
                value: format!(
                    "{:.11}{}({:.8}% of total)",
                    amount_to_ui_amount(stake.balance, mint.decimals),
                    symbol,
                    (stake.balance as f64 / boost.total_stake as f64) * 100f64
                ),
            });
            data.push(TableData {
                key: "Deposits (pending)".to_string(),
                value: format!(
                    "{}{}({:.8}% of total)",
                    amount_to_ui_amount(stake.pending_balance, mint.decimals),
                    symbol,
                    (stake.pending_balance as f64 / boost.total_stake as f64) * 100f64
                ),
            });
            data.push(TableData {
                key: "Last deposit at".to_string(),
                value: format_timestamp(stake.last_deposit_at),
            });
            data.push(TableData {
                key: "Yield".to_string(),
                value: if stake.rewards > 0 {
                    format!("{:.11} ORE", amount_u64_to_f64(stake.rewards)).yellow().bold().to_string()
                } else {
                    format!("{:.11} ORE", amount_u64_to_f64(stake.rewards))
                },
            });
        } else {
            data.push(TableData {
                key: "Status".to_string(),
                value: "Not found".red().bold().to_string(),
            });
        }
    }

    async fn fetch_boost_data(&self, address: Pubkey, boost: Boost, mint: Mint, symbol: String, data: &mut Vec<TableData>) {
        let boost_proof_address = proof_pda(address).0;
        let boost_proof: ore_api::prelude::Proof = get_proof(&self.rpc_client, boost_proof_address).await.unwrap();
        data.push(TableData {
            key: "Address".to_string(),
            value: address.to_string(),
        });
        data.push(TableData {
            key: "Expires at".to_string(),
            value: format_timestamp(boost.expires_at),
        });
        data.push(TableData {
            key: "Locked".to_string(),
            value: boost.locked.to_string(),
        });
        data.push(TableData {
            key: "Mint".to_string(),
            value: boost.mint.to_string(),
        });
        data.push(TableData {
            key: "Multiplier".to_string(),
            value: format!(
                "{}x",
                boost.multiplier as f64 / BOOST_DENOMINATOR as f64
            ),
        });
        data.push(TableData {
            key: "Total deposits".to_string(),
            value: format!(
                "{}{}",
                amount_to_ui_amount(boost.total_stake, mint.decimals),
                symbol.trim_end_matches(' ')
            ),
        });
        data.push(TableData { 
            key: "Total stakers".to_string(), 
            value: boost.total_stakers.to_string()
        });
        data.push(TableData {
            key: "Yield (pending)".to_string(),
            value: format!(
                "{:.11} ORE",
                amount_to_ui_amount(boost_proof.balance, ore_api::consts::TOKEN_DECIMALS)
            ),
        });
    }

    async fn stake_list(&self) -> Result<(), Error> {
        // Iterate over all boosts
        let mut data = vec![];
        let boosts = get_boosts(&self.rpc_client).await.unwrap();
        for (address, boost) in boosts {

            // Get relevant accounts
            let stake_address = stake_pda(self.signer().pubkey(), address).0;
            let stake = get_stake(&self.rpc_client, stake_address).await;
            let mint = get_mint(&self.rpc_client, boost.mint).await.unwrap();
            let metadata_address = mpl_token_metadata::accounts::Metadata::find_pda(&boost.mint).0;
            let symbol = match self.rpc_client.get_account_data(&metadata_address).await {
                Err(_) => " ".to_string(),
                Ok(metadata_data) => {
                    if let Ok(metadata) = mpl_token_metadata::accounts::Metadata::from_bytes(&metadata_data) {
                        format!(" {} ", metadata.symbol)
                    } else {
                        " ".to_string()
                    }
                }
            };

            // Parse optional stake data
            let (stake_balance, stake_rewards) = if let Ok(stake) = stake {
                (stake.balance, stake.rewards)
            } else {
                (0, 0)
            };

            // Aggregate data
            data.push(StakeTableData {
                mint: boost.mint.to_string(),
                multiplier: format!("{}x", boost.multiplier as f64 / BOOST_DENOMINATOR as f64),
                total_deposits: format!("{:.11}{}", amount_to_ui_amount(boost.total_stake, mint.decimals), symbol.trim_end_matches(' ')),
                total_stakers: boost.total_stakers.to_string(),
                my_deposits: format!("{:.11}{}", amount_to_ui_amount(stake_balance, mint.decimals), symbol.trim_end_matches(' ')),
                my_share: format!("{:.8}%", (stake_balance as f64 / boost.total_stake as f64) * 100f64),
                my_yield: if stake_rewards > 0 {
                    format!("{:.11} ORE", amount_u64_to_f64(stake_rewards)).yellow().bold().to_string()
                } else {
                    format!("{:.11} ORE", amount_u64_to_f64(stake_rewards))
                },
            });
        }

        // Build table
        let mut table = Table::new(data);
        table.with(Style::blank());
        table.modify(Columns::new(1..), Alignment::right());
        table.modify(Rows::first(), Color::BOLD);
        table.with(Highlight::new(Rows::single(1)).color(BorderColor::default().top(Color::FG_WHITE)));
        table.with(Highlight::new(Rows::single(1)).border(Border::new().top('━')));
        println!("\n{table}\n");
        Ok(())
    }

    async fn stake_deposit(
        &self,
        args: StakeDepositArgs,
        stake_args: StakeArgs,
    ) -> Result<(), Error> {
        // Parse mint address
        let mint_address = Pubkey::from_str(&stake_args.mint.unwrap()).unwrap();

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
        let Ok(mint_data) = self.rpc_client.get_account_data(&mint_address).await else {
            println!("Failed to fetch mint account");
            return Ok(());
        };
        let mint = Mint::unpack(&mint_data).unwrap();
        let Ok(Some(token_account)) = self.rpc_client.get_token_account(&sender).await else {
            println!("Failed to fetch token account");
            return Ok(());
        };

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
        if let Err(err) = get_boost(&self.rpc_client, boost_address).await {
            println!("Failed to fetch boost account: {}", err);
            return Ok(());
        }

        // Open stake account, if needed
        if let Err(_err) = self.rpc_client.get_account_data(&stake_address).await {
            println!("Initializing stake account...");
            let ix = ore_boost_api::sdk::open(signer.pubkey(), signer.pubkey(), mint_address);
            match self.send_and_confirm(&[ix], ComputeBudget::Fixed(32_000), false).await {
                Err(e) => println!("Failed to initialize stake account: {}", e),
                Ok(sig) => println!("{} {}", "OK".green().bold(), sig),
            }
        }

        // Send tx
        println!("Depositing stake...");
        let ix = ore_boost_api::sdk::deposit(signer.pubkey(), mint_address, amount);
        match self.send_and_confirm(&[ix], ComputeBudget::Fixed(32_000), false).await {
            Err(e) => println!("Failed to deposit stake: {}", e),
            Ok(sig) => println!("{} {}", "OK".green().bold(), sig),
        }

        Ok(())
    }

    async fn stake_withdraw(
        &self,
        args: StakeWithdrawArgs,
        stake_args: StakeArgs,
    ) -> Result<(), Error> {
        // Parse mint address
        let mint_address = Pubkey::from_str(&stake_args.mint.unwrap()).unwrap();

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
        let stake = get_stake(&self.rpc_client, stake_address).await.unwrap();
        
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

}


impl Miner {
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
        let mint_address = Pubkey::from_str(&stake_args.mint.unwrap()).unwrap();
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
        let mint_address = Pubkey::from_str(&stake_args.mint.unwrap()).unwrap();
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