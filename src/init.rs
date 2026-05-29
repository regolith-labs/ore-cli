use pqcrypto_falcon::falcon512;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::Signer,
    signer::keypair::read_keypair_file,
    transaction::Transaction,
};
use spl_associated_token_account::{
    get_associated_token_address,
    instruction::create_associated_token_account,
};

use crate::config::OreConfig;
use crate::keypair::{save_keypair, vector_pda, wire_pubkey};
use crate::tx::send_and_confirm;

const ORE_MINT: &str = "oreoU2P8bN6jkk3jbaiVxYnG1dCXcYxwhwyK9jSybcp";

pub fn run() -> anyhow::Result<()> {
    let cfg = OreConfig::load();
    let kp_path = cfg.keypair_path();

    // Generate or load keypair
    let (pk, _sk) = if kp_path.exists() {
        println!("Loading existing keypair from {}", kp_path.display());
        crate::keypair::load_keypair(&kp_path)?
    } else {
        println!("Generating new keypair...");
        let (pk, sk) = falcon512::keypair();
        save_keypair(&kp_path, &pk, &sk)?;
        println!("Keypair saved to {}", kp_path.display());
        (pk, sk)
    };

    let wire = wire_pubkey(&pk);
    let (vector_addr, _bump) = vector_pda(&pk);

    // Convert vector-core Address to solana-sdk Pubkey
    let vector_pubkey = Pubkey::new_from_array(vector_addr.to_bytes());
    let ore_mint = ORE_MINT.parse::<Pubkey>()?;

    println!("Smart wallet: {vector_pubkey}");

    // Load fee payer
    let fee_payer = read_keypair_file(cfg.fee_payer_path())
        .map_err(|e| anyhow::anyhow!("failed to read fee payer keypair: {e}"))?;
    let rpc = RpcClient::new(cfg.rpc_url());

    // 1. Initialize smart wallet (if not already initialized)
    let vector_account = rpc.get_account(&vector_pubkey);
    if vector_account.is_err() {
        let payer_addr = solana_address::Address::new_from_array(fee_payer.pubkey().to_bytes());
        let init_ix = vector_core::create_initialize_falcon512(&payer_addr, &wire);
        let ix = to_sdk_instruction(init_ix);

        let recent_blockhash = rpc.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&fee_payer.pubkey()),
            &[&fee_payer],
            recent_blockhash,
        );
        send_and_confirm(&rpc, &tx, "Initializing smart wallet...")?;
    } else {
        println!("Smart wallet already exists.");
    }

    // 2. Create ORE ATA for the smart wallet (if not already created)
    let ata = get_associated_token_address(&vector_pubkey, &ore_mint);
    let ata_account = rpc.get_account(&ata);
    if ata_account.is_err() {
        let create_ata_ix = create_associated_token_account(
            &fee_payer.pubkey(),
            &vector_pubkey,
            &ore_mint,
            &spl_token::id(),
        );
        let recent_blockhash = rpc.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[create_ata_ix],
            Some(&fee_payer.pubkey()),
            &[&fee_payer],
            recent_blockhash,
        );
        send_and_confirm(&rpc, &tx, "Creating ORE token account...")?;
    } else {
        println!("ORE token account already exists.");
    }

    // 3. Create Address Lookup Table (if not already created)
    let mut cfg = OreConfig::load(); // reload in case it was updated
    if cfg.lookup_table.is_none() {
        let ore_stake_program: Pubkey = "STkEAu2cEyQp5ktgUauRVq8es6mEP2w6ixw4NEd5tDJ".parse()?;
        let stake_pda = Pubkey::find_program_address(
            &[b"stake", &vector_pubkey.to_bytes()],
            &ore_stake_program,
        ).0;
        let treasury_pda = Pubkey::find_program_address(&[b"treasury"], &ore_stake_program).0;
        let vesting_pda = Pubkey::find_program_address(&[b"vesting"], &ore_stake_program).0;
        let stake_tokens = spl_associated_token_account::get_associated_token_address(&stake_pda, &ore_mint);
        let instructions_sysvar: Pubkey = "Sysvar1nstructions1111111111111111111111111".parse()?;

        let addresses = vec![
            vector_pubkey,
            instructions_sysvar,
            ore_mint,
            ata,
            ore_stake_program,
            stake_pda,
            stake_tokens,
            treasury_pda,
            vesting_pda,
            solana_sdk::system_program::id(),
            spl_token::id(),
            spl_associated_token_account::id(),
        ];

        let alt_address = crate::lookup_table::create_and_populate(&rpc, &fee_payer, &addresses)?;
        cfg.lookup_table = Some(alt_address.to_string());
        cfg.save()?;
    } else {
        println!("Lookup table already exists.");
    }

    println!();
    println!("Smart wallet initialized!");
    println!("  Address: {vector_pubkey}");

    Ok(())
}

/// Convert a vector-core `solana_instruction::Instruction` to a `solana_sdk::instruction::Instruction`.
pub fn to_sdk_instruction(ix: solana_instruction::Instruction) -> solana_sdk::instruction::Instruction {
    solana_sdk::instruction::Instruction {
        program_id: Pubkey::new_from_array(ix.program_id.to_bytes()),
        accounts: ix
            .accounts
            .into_iter()
            .map(|m| solana_sdk::instruction::AccountMeta {
                pubkey: Pubkey::new_from_array(m.pubkey.to_bytes()),
                is_signer: m.is_signer,
                is_writable: m.is_writable,
            })
            .collect(),
        data: ix.data,
    }
}
