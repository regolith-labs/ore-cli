use std::str::FromStr;

use solana_program::address_lookup_table;
use solana_sdk::signature::Signer;
use kliquidity_sdk::accounts::WhirlpoolStrategy;
use steel::Pubkey;

use crate::{Miner, utils::ComputeBudget, args::LutArgs};

impl Miner {
    pub async fn lut_kamino(&self, args: LutArgs) {
        let authority = self.signer().pubkey();

        // Get kamino strategy
        let strategy_address = Pubkey::from_str(&args.strategy).unwrap();
        let account_data = self.rpc_client.get_account_data(&strategy_address).await.unwrap();
        let strategy = WhirlpoolStrategy::from_bytes(&mut &account_data[..]).unwrap();

        // Get current slot
        let slot = self.rpc_client.get_slot().await.unwrap();

        // Create lookup table
        let (create_lut_ix, lut_address) = address_lookup_table::instruction::create_lookup_table_signed(
            authority,
            authority,
            slot,
        );

        // Extend lookup table
        let extend_lut_ix = address_lookup_table::instruction::extend_lookup_table(
            lut_address,
            authority,
            Some(authority),
            vec![
                strategy_address,
                strategy.global_config,
                strategy.pool,
                strategy.position,
                strategy.tick_array_lower,
                strategy.tick_array_upper,
                strategy.token_a_vault,
                strategy.token_b_vault,
                strategy.base_vault_authority,
                strategy.pool_token_vault_a,
                strategy.pool_token_vault_b,
                strategy.token_a_mint,
                strategy.token_b_mint,
                strategy.shares_mint,
                strategy.position_token_account
            ],
        );

        // Program addresses that (maybe?) could be added to the lookup table
        //
        // 11111111111111111111111111111111, 
        // BoosTyJFPPtrqJTdi49nnztoEWDJXfDRhyb2fha6PPy, 
        // MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr, 
        // Sysvar1nstructions1111111111111111111111111, 
        // TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA, 
        // TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb, 
        // whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc, 
        // 6LtLpnUFNByNXLyCoK9wA2MykKAmQNZKBdY8s47dehDc, 
        // ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL

        println!("Lookup table address: {}", lut_address);

        // Submit tx
        let ixs = vec![create_lut_ix, extend_lut_ix];
        let _ = self.send_and_confirm(&ixs, ComputeBudget::Fixed(1_000_000), false).await.unwrap();
    }
}
