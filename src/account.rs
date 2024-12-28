use std::str::FromStr;

use colored::Colorize;
use ore_api::state::proof_pda;
use owo_colors::OwoColorize;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use spl_token::amount_to_ui_amount;
use tabled::{Table, settings::{Style, Remove, object::{Rows, Columns}, Color, Highlight, style::{LineText, BorderColor}, Border, Alignment}};

use crate::{
    args::{AccountArgs, AccountCommand, ClaimArgs, AccountCloseArgs},
    utils::{get_proof, format_timestamp, get_proof_with_authority, ask_confirm, TableData},
    Miner, send_and_confirm::ComputeBudget,
};

impl Miner {
    pub async fn account(&self, args: AccountArgs) {
        if let Some(command) = args.command {
            match command {
                AccountCommand::Close(args) => self.close(args).await,
            }
        } else {
            self.get_account(args).await;
        }
    }

    pub async fn get_account(&self, args: AccountArgs) {
        // Parse account address
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

        // Aggregate data   
        let mut data = vec![];
        self.get_account_data(address, &mut data).await;
        self.get_proof_data(address, &mut data).await;

        // Build table
        let mut table = Table::new(data);
        table.with(Remove::row(Rows::first()));
        table.modify(Columns::single(1), Alignment::right());
        table.with(Style::blank());
        let title_color = Color::try_from(" ".bold().black().on_white().to_string()).unwrap();
        
        // Account title
        table.with(Highlight::new(Rows::first()).color(BorderColor::default().top(Color::FG_WHITE)));
        table.with(Highlight::new(Rows::first()).border(Border::new().top('━')));
        table.with(LineText::new("Account", Rows::first()).color(title_color.clone()));

        // Proof title
        table.with(Highlight::new(Rows::single(2)).color(BorderColor::default().top(Color::FG_WHITE)));
        table.with(Highlight::new(Rows::single(2)).border(Border::new().top('━')));
        table.with(LineText::new("Proof", Rows::single(2)).color(title_color.clone()));
 
        println!("{table}\n");
    }

    async fn get_account_data(&self, authority: Pubkey, data: &mut Vec<TableData>) {
        // Get balance
        let token_account_address = spl_associated_token_account::get_associated_token_address(
            &authority,
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

        // Aggregate data
        data.push(TableData {
            key: "Address".to_string(),
            value: authority.to_string(),
        });
        data.push(TableData {
            key: "Balance".to_string(),
            value: format!("{} ORE", token_balance),
        });
    }

    async fn get_proof_data(&self, authority: Pubkey, data: &mut Vec<TableData>) {
        // Parse addresses
        let proof_address = proof_pda(authority).0;
        let proof = get_proof(&self.rpc_client, proof_address).await;

        // Aggregate data
        data.push(TableData {
            key: "Address".to_string(),
            value: proof_address.to_string(),
        });
        if let Ok(proof) = proof {
            data.push(TableData {
                key: "Last hash".to_string(),
                value: solana_sdk::hash::Hash::new_from_array(proof.last_hash).to_string(),
            });
            data.push(TableData {
                key: "Last hash at".to_string(),
                value: format_timestamp(proof.last_hash_at),
            });
            data.push(TableData {
                key: "Miner".to_string(),
                value: proof.miner.to_string(),
            });
            data.push(TableData {
                key: "Total hashes".to_string(),
                value: proof.total_hashes.to_string(),
            });
            data.push(TableData {
                key: "Total rewards".to_string(),
                value: format!("{} ORE", amount_to_ui_amount(proof.total_rewards, ore_api::consts::TOKEN_DECIMALS)),
            });
            data.push(TableData {
                key: "Unclaimed".to_string(),
                value: format!("{} ORE", amount_to_ui_amount(proof.balance, ore_api::consts::TOKEN_DECIMALS)),
            });
        } else {
            data.push(TableData {
                key: "Status".to_string(),
                value: "Not found".red().bold().to_string(),
            });
        }
    }

    async fn close(&self, _args: AccountCloseArgs) {
        // Confirm proof exists
        let signer = self.signer();
        let proof = get_proof_with_authority(&self.rpc_client, signer.pubkey()).await.unwrap();

        // Confirm the user wants to close.
        if !ask_confirm(
            format!("{} You have {} ORE staked in this account.\nAre you sure you want to {}close this account? [Y/n]", 
                "WARNING".yellow(),
                amount_to_ui_amount(proof.balance, ore_api::consts::TOKEN_DECIMALS),
                if proof.balance.gt(&0) { "claim your stake and "} else { "" }
            ).as_str()
        ) {
            return;
        }

        // Claim stake
        if proof.balance.gt(&0) {
            self.claim_from_proof(ClaimArgs {
                amount: None,
                to: None,
                pool_url: None,
            })
            .await;
        }

        // Submit close transaction
        let ix = ore_api::sdk::close(signer.pubkey());
        self.send_and_confirm(&[ix], ComputeBudget::Fixed(500_000), false)
            .await
            .ok();
    }
}
