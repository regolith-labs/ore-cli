#![allow(deprecated)] // solana_sdk::address_lookup_table is deprecated but functional

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    address_lookup_table::{
        self,
        state::AddressLookupTable,
        AddressLookupTableAccount,
    },
    pubkey::Pubkey,
    signature::Signer,
    signer::keypair::Keypair,
    transaction::Transaction,
};

use crate::tx::send_and_confirm;

/// Create an Address Lookup Table and populate it with the given addresses.
/// Returns the ALT address.
pub fn create_and_populate(
    rpc: &RpcClient,
    fee_payer: &Keypair,
    addresses: &[Pubkey],
) -> anyhow::Result<Pubkey> {
    let recent_slot = rpc.get_slot()?.saturating_sub(2);

    // 1. Create the lookup table
    let (create_ix, alt_address) = address_lookup_table::instruction::create_lookup_table(
        fee_payer.pubkey(),
        fee_payer.pubkey(),
        recent_slot,
    );

    let blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[create_ix],
        Some(&fee_payer.pubkey()),
        &[fee_payer],
        blockhash,
    );
    send_and_confirm(rpc, &tx, "Creating address lookup table...")?;

    // 2. Extend with addresses
    for chunk in addresses.chunks(20) {
        let extend_ix = address_lookup_table::instruction::extend_lookup_table(
            alt_address,
            fee_payer.pubkey(),
            Some(fee_payer.pubkey()),
            chunk.to_vec(),
        );
        let blockhash = rpc.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[extend_ix],
            Some(&fee_payer.pubkey()),
            &[fee_payer],
            blockhash,
        );
        send_and_confirm(rpc, &tx, "Populating lookup table...")?;
    }

    Ok(alt_address)
}

/// Fetch an AddressLookupTableAccount from on-chain data.
pub fn fetch_alt(
    rpc: &RpcClient,
    alt_address: &Pubkey,
) -> anyhow::Result<AddressLookupTableAccount> {
    let account = rpc.get_account(alt_address)?;
    let table = AddressLookupTable::deserialize(&account.data)?;
    Ok(AddressLookupTableAccount {
        key: *alt_address,
        addresses: table.addresses.to_vec(),
    })
}
