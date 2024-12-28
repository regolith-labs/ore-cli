use std::str::FromStr;

use colored::Colorize;
use ore_api::state::proof_pda;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use spl_token::amount_to_ui_amount;
use tabled::{Tabled, Table, settings::{Style, Rotate, Panel}};

use crate::{
    args::{AccountArgs, AccountCommand, ClaimArgs, AccountCloseArgs},
    utils::{get_proof, format_timestamp, get_proof_with_authority, ask_confirm},
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
        #[derive(Tabled)]
        struct AccountData {
            address: String,
            balance: String,
            last_solution: String,
            last_solution_at: String,
            lifetime_solutions: String,
            miner: String,
            proof: String,
        }
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
        let proof_address = proof_pda(address).0;
        let proof = get_proof(&self.rpc_client, proof_address).await;
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
        let mut data: Vec<AccountData> = vec![];
        data.push(AccountData {
            address: address.to_string(),
            balance: token_balance,
            last_solution: solana_sdk::hash::Hash::new_from_array(proof.last_hash).to_string(),
            last_solution_at: format_timestamp(proof.last_hash_at),
            lifetime_solutions: proof.total_hashes.to_string(),
            miner: proof.miner.to_string(),
            proof: proof_address.to_string(),
        });
        let mut table = Table::new(data);
        table.with(Rotate::Left);
        // table.with(Panel::header("Account"));
        table.with(Style::modern());
        println!("{}\n", table);
    }

    async fn close(&self, _args: AccountCloseArgs) {
        // Confirm proof exists
        let signer = self.signer();
        let proof = get_proof_with_authority(&self.rpc_client, signer.pubkey()).await;

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
