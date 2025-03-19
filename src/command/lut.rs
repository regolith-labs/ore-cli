use std::str::FromStr;

use kliquidity_sdk::accounts::WhirlpoolStrategy;
use solana_program::address_lookup_table;
use solana_sdk::signature::Signer;
use steel::Pubkey;

use crate::{args::LutArgs, utils::ComputeBudget, Miner};

impl Miner {
    pub async fn lut_kamino(&self, args: LutArgs) {
        let authority = self.signer().pubkey();

        // Get kamino strategy
        // let strategy_address = Pubkey::from_str(&args.strategy).unwrap();
        // let account_data = self
        //     .rpc_client
        //     .get_account_data(&strategy_address)
        //     .await
        //     .unwrap();
        // let strategy = WhirlpoolStrategy::from_bytes(&mut &account_data[..]).unwrap();

        // Get current slot
        let slot = self.rpc_client.get_slot().await.unwrap();

        // Create lookup table
        let (create_lut_ix, lut_address) =
            address_lookup_table::instruction::create_lookup_table_signed(
                authority, authority, slot,
            );

        // Extend lookup table
        let extend_lut_ix = address_lookup_table::instruction::extend_lookup_table(
            lut_address,
            authority,
            Some(authority),
            vec![
                Pubkey::from_str("8BEzwBTDsKWnjgjxi8Cca7ZatPZQhxUMgS8qWzBhDhrC").unwrap(), // boost
                Pubkey::from_str("Dh5ZkjGD8EVujR7C8mxMyYaE2LRVarJ9W6bMofTgNJFP").unwrap(), // treasury
                Pubkey::from_str("HqPcY2CUB4FL5EAGWN1yZkS6DHYUoMsnjoSpdGqV8wPC").unwrap(), // treasury tokens
                Pubkey::from_str("7XNR3Ysqg2MbfQX8iMWD4iEF96h2GMsWNT8eZYsLTmua").unwrap(),
                Pubkey::from_str("9BAWwtAZiF4XJC6vArPM8JhtgKXfeoeo9FJHeR3PEGac").unwrap(),
                Pubkey::from_str("A9Nt1w73vS1W7kphM3ykoqYqunjq86a18LWcegXWDewk").unwrap(),
                Pubkey::from_str("3YiQeRH8i4fopVYHXwrpZ55chmr77dYhCvsmdAuPyEQg").unwrap(),
                Pubkey::from_str("3s6ki6dQSM8FuqWiPsnGkgVsAEo8BTAfUR1Vvt1TPiJN").unwrap(),
                Pubkey::from_str("3ESUFCnRNgZ7Mn2mPPUMmXYaKU8jpnV9VtA17M7t2mHQ").unwrap(),
                Pubkey::from_str("6Av9sdKvnjwoDHVnhEiz6JEq8e6SGzmhCsCncT2WJ7nN").unwrap(),
                Pubkey::from_str("3RpEekjLE5cdcG15YcXJUpxSepemvq2FpmMcgo342BwC").unwrap(),
                Pubkey::from_str("BtJuiRG44vew5nYBVeUhuBawPTZLyYYxdzTYzerkfnto").unwrap(),
                Pubkey::from_str("C2QoQ111jGHEy5918XkNXQro7gGwC9PKLXd1LqBiYNwA").unwrap(),
                // strategy_address,
                // strategy.global_config,
                // strategy.pool,
                // strategy.position,
                // strategy.tick_array_lower,
                // strategy.tick_array_upper,
                // strategy.token_a_vault,
                // strategy.token_b_vault,
                // strategy.base_vault_authority,
                // strategy.pool_token_vault_a,
                // strategy.pool_token_vault_b,
                // strategy.token_a_mint,
                // strategy.token_b_mint,
                // strategy.shares_mint,
                // strategy.position_token_account,
            ],
        );

        // Submit tx
        let ixs = vec![create_lut_ix, extend_lut_ix];
        let _ = self
            .send_and_confirm(&ixs, ComputeBudget::Fixed(1_000_000), false)
            .await
            .unwrap();

        // Extend lookup table
        // let extend_lut_ix = address_lookup_table::instruction::extend_lookup_table(
        //     lut_address,
        //     authority,
        //     Some(authority),
        //     vec![
        //         Pubkey::from_str("11111111111111111111111111111111").unwrap(),
        //         Pubkey::from_str("BoostzzkNfCA9D1qNuN5xZxB5ErbK4zQuBeTHGDpXT1").unwrap(),
        //         Pubkey::from_str("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr").unwrap(),
        //         Pubkey::from_str("Sysvar1nstructions1111111111111111111111111").unwrap(),
        //         Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap(),
        //         Pubkey::from_str("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb").unwrap(),
        //         Pubkey::from_str("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc").unwrap(),
        //         Pubkey::from_str("6LtLpnUFNByNXLyCoK9wA2MykKAmQNZKBdY8s47dehDc").unwrap(),
        //         Pubkey::from_str("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL").unwrap(),
        //     ],
        // );

        // // Submit tx
        // let ixs = vec![extend_lut_ix];
        // let _ = self
        //     .send_and_confirm(&ixs, ComputeBudget::Fixed(1_000_000), false)
        //     .await
        //     .unwrap();

        println!("Lookup table address: {}", lut_address);
    }
}
