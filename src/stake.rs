use pqcrypto_falcon::falcon512;
use pqcrypto_traits::sign::DetachedSignature;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::Signer,
    signer::keypair::read_keypair_file,
};
use spl_associated_token_account::get_associated_token_address;
use vector_core::{
    advance_vector_digest, create_advance_instruction, create_passthrough_instruction,
    FALCON512, FALCON512_SIGNATURE_LEN, VectorAccount,
};

use crate::config::OreConfig;
use crate::init::to_sdk_instruction;
use crate::keypair::{load_keypair, falcon_identity, vector_pda};

const ORE_MINT: &str = "oreoU2P8bN6jkk3jbaiVxYnG1dCXcYxwhwyK9jSybcp";
const ORE_DECIMALS: u8 = 11;
const ORE_STAKE_PROGRAM: &str = "STkEAu2cEyQp5ktgUauRVq8es6mEP2w6ixw4NEd5tDJ";
const STAKE_SEED: &[u8] = b"stake";
const TREASURY_SEED: &[u8] = b"treasury";
const VESTING_SEED: &[u8] = b"vesting";

// Instruction discriminators (from ore-stake-api)
const DEPOSIT_DISC: u8 = 10;
const WITHDRAW_DISC: u8 = 11;
const CLAIM_DISC: u8 = 12;

fn ore_stake_program_id() -> Pubkey {
    ORE_STAKE_PROGRAM.parse().unwrap()
}

fn stake_pda(authority: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[STAKE_SEED, &authority.to_bytes()], &ore_stake_program_id()).0
}

fn treasury_pda() -> Pubkey {
    Pubkey::find_program_address(&[TREASURY_SEED], &ore_stake_program_id()).0
}

fn vesting_pda() -> Pubkey {
    Pubkey::find_program_address(&[VESTING_SEED], &ore_stake_program_id()).0
}

fn format_ore(amount: u64) -> String {
    let whole = amount / 10u64.pow(ORE_DECIMALS as u32);
    let frac = amount % 10u64.pow(ORE_DECIMALS as u32);
    if frac == 0 {
        format!("{whole} ORE")
    } else {
        let frac_str = format!("{frac:0>width$}", width = ORE_DECIMALS as usize);
        let trimmed = frac_str.trim_end_matches('0');
        format!("{whole}.{trimmed} ORE")
    }
}

/// Convert a solana_sdk::Pubkey to a solana_address::Address (for vector-core).
fn to_addr(pubkey: &Pubkey) -> solana_address::Address {
    solana_address::Address::new_from_array(pubkey.to_bytes())
}

/// Parse a human-readable ORE amount into base units.
fn parse_ore_amount(s: &str) -> anyhow::Result<u64> {
    let parts: Vec<&str> = s.split('.').collect();
    match parts.len() {
        1 => {
            let whole: u64 = parts[0].parse()?;
            Ok(whole * 10u64.pow(ORE_DECIMALS as u32))
        }
        2 => {
            let whole: u64 = parts[0].parse()?;
            let frac_str = parts[1];
            if frac_str.len() > ORE_DECIMALS as usize {
                anyhow::bail!("too many decimal places (max {ORE_DECIMALS})");
            }
            let frac: u64 = if frac_str.is_empty() {
                0
            } else {
                let padded = format!("{:0<width$}", frac_str, width = ORE_DECIMALS as usize);
                padded.parse()?
            };
            Ok(whole * 10u64.pow(ORE_DECIMALS as u32) + frac)
        }
        _ => anyhow::bail!("invalid amount format: {s}"),
    }
}

/// Sign an advance+passthrough transaction and submit it using a v0 transaction
/// with an Address Lookup Table for size compression.
fn sign_and_send(
    cfg: &OreConfig,
    identity: &[u8; 32],
    sk: &falcon512::SecretKey,
    sub_ix: solana_instruction::Instruction,
) -> anyhow::Result<()> {
    let fee_payer = read_keypair_file(cfg.fee_payer_path())
        .map_err(|e| anyhow::anyhow!("failed to read fee payer keypair: {e}"))?;
    let rpc = RpcClient::new(cfg.rpc_url());

    let (vector_addr, _bump) = vector_core::find_vector_pda(&FALCON512, identity);
    let vector_pubkey = Pubkey::new_from_array(vector_addr.to_bytes());

    // Fetch current nonce
    let vector_account_data = rpc.get_account_data(&vector_pubkey)?;
    if vector_account_data.len() < VectorAccount::HEADER_LEN {
        anyhow::bail!("Smart wallet not initialized — run `ore init` first.");
    }
    let header: [u8; VectorAccount::HEADER_LEN] =
        vector_account_data[..VectorAccount::HEADER_LEN].try_into()?;
    let nonce = VectorAccount::from_header_bytes(&header).nonce;

    // Build passthrough
    let passthrough_ix = create_passthrough_instruction(&FALCON512, identity, &[sub_ix]);

    // Compute digest and sign
    let digest = advance_vector_digest(
        &FALCON512,
        &nonce,
        identity,
        &[],
        std::slice::from_ref(&passthrough_ix),
    );
    let raw_sig = falcon512::detached_sign(&digest, sk);
    let sig_bytes = raw_sig.as_bytes();
    let mut signature = [0u8; FALCON512_SIGNATURE_LEN];
    if sig_bytes.len() > FALCON512_SIGNATURE_LEN {
        anyhow::bail!("Falcon-512 signature too long");
    }
    signature[..sig_bytes.len()].copy_from_slice(sig_bytes);

    let advance_ix = create_advance_instruction(&FALCON512, identity, &signature);

    let sdk_advance = to_sdk_instruction(advance_ix);
    let sdk_passthrough = to_sdk_instruction(passthrough_ix);

    // Load the ALT for v0 transaction compression
    let alt_address_str = cfg.lookup_table.as_ref()
        .ok_or_else(|| anyhow::anyhow!("No lookup table configured — run `ore init` first."))?;
    let alt_address: Pubkey = alt_address_str.parse()?;
    let alt = crate::lookup_table::fetch_alt(&rpc, &alt_address)?;

    let recent_blockhash = rpc.get_latest_blockhash()?;
    let msg = solana_sdk::message::v0::Message::try_compile(
        &fee_payer.pubkey(),
        &[sdk_advance, sdk_passthrough],
        &[alt],
        recent_blockhash,
    )?;
    let versioned_msg = solana_sdk::message::VersionedMessage::V0(msg);
    let tx = solana_sdk::transaction::VersionedTransaction::try_new(versioned_msg, &[&fee_payer])?;

    crate::tx::send_and_confirm_versioned(&rpc, &tx, "Submitting transaction...")?;
    Ok(())
}

/// Build the ore-stake deposit instruction as a solana_instruction::Instruction
/// with the Vector PDA as signer/payer (is_signer: false for passthrough).
fn build_deposit_ix(
    wallet: &Pubkey,
    fee_payer: &Pubkey,
    amount: u64,
    compound_fee: u64,
    compound_fee_deposit: u64,
) -> solana_instruction::Instruction {
    let ore_mint: Pubkey = ORE_MINT.parse().unwrap();
    let stake = stake_pda(wallet);
    let stake_tokens = get_associated_token_address(&stake, &ore_mint);
    let sender_ata = get_associated_token_address(wallet, &ore_mint);
    let treasury = treasury_pda();
    let vesting = vesting_pda();
    let program_id = ore_stake_program_id();

    // Data: [disc(1), amount(8), compound_fee(8), compound_fee_deposit(8)]
    let mut data = Vec::with_capacity(25);
    data.push(DEPOSIT_DISC);
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&compound_fee.to_le_bytes());
    data.extend_from_slice(&compound_fee_deposit.to_le_bytes());

    solana_instruction::Instruction {
        program_id: to_addr(&program_id),
        accounts: vec![
            solana_instruction::AccountMeta::new(to_addr(wallet), false),     // signer/authority (via PDA CPI)
            solana_instruction::AccountMeta::new(to_addr(fee_payer), true),   // payer (tx-level signer, propagates through CPI)
            solana_instruction::AccountMeta::new_readonly(to_addr(&ore_mint), false),
            solana_instruction::AccountMeta::new(to_addr(&sender_ata), false),
            solana_instruction::AccountMeta::new(to_addr(&stake), false),
            solana_instruction::AccountMeta::new(to_addr(&stake_tokens), false),
            solana_instruction::AccountMeta::new(to_addr(&treasury), false),
            solana_instruction::AccountMeta::new(to_addr(&vesting), false),
            solana_instruction::AccountMeta::new_readonly(to_addr(&solana_sdk::system_program::id()), false),
            solana_instruction::AccountMeta::new_readonly(to_addr(&spl_token::id()), false),
            solana_instruction::AccountMeta::new_readonly(to_addr(&spl_associated_token_account::id()), false),
            solana_instruction::AccountMeta::new_readonly(to_addr(&program_id), false),
        ],
        data,
    }
}

/// Build the ore-stake withdraw instruction as a solana_instruction::Instruction.
fn build_withdraw_ix(wallet: &Pubkey, amount: u64) -> solana_instruction::Instruction {
    let ore_mint: Pubkey = ORE_MINT.parse().unwrap();
    let stake = stake_pda(wallet);
    let stake_tokens = get_associated_token_address(&stake, &ore_mint);
    let recipient_ata = get_associated_token_address(wallet, &ore_mint);
    let treasury = treasury_pda();
    let vesting = vesting_pda();
    let program_id = ore_stake_program_id();

    // Data: [disc(1), amount(8)]
    let mut data = Vec::with_capacity(9);
    data.push(WITHDRAW_DISC);
    data.extend_from_slice(&amount.to_le_bytes());

    solana_instruction::Instruction {
        program_id: to_addr(&program_id),
        accounts: vec![
            solana_instruction::AccountMeta::new(to_addr(wallet), false),  // signer (via CPI)
            solana_instruction::AccountMeta::new_readonly(to_addr(&ore_mint), false),
            solana_instruction::AccountMeta::new(to_addr(&recipient_ata), false),
            solana_instruction::AccountMeta::new(to_addr(&stake), false),
            solana_instruction::AccountMeta::new(to_addr(&stake_tokens), false),
            solana_instruction::AccountMeta::new(to_addr(&treasury), false),
            solana_instruction::AccountMeta::new(to_addr(&vesting), false),
            solana_instruction::AccountMeta::new_readonly(to_addr(&solana_sdk::system_program::id()), false),
            solana_instruction::AccountMeta::new_readonly(to_addr(&spl_token::id()), false),
            solana_instruction::AccountMeta::new_readonly(to_addr(&spl_associated_token_account::id()), false),
            solana_instruction::AccountMeta::new_readonly(to_addr(&program_id), false),
        ],
        data,
    }
}

/// Build the ore-stake claim instruction.
fn build_claim_ix(wallet: &Pubkey, amount: u64) -> solana_instruction::Instruction {
    let ore_mint: Pubkey = ORE_MINT.parse().unwrap();
    let stake = stake_pda(wallet);
    let recipient_ata = get_associated_token_address(wallet, &ore_mint);
    let treasury = treasury_pda();
    let treasury_tokens = get_associated_token_address(&treasury, &ore_mint);
    let vesting = vesting_pda();
    let program_id = ore_stake_program_id();

    let mut data = Vec::with_capacity(9);
    data.push(CLAIM_DISC);
    data.extend_from_slice(&amount.to_le_bytes());

    solana_instruction::Instruction {
        program_id: to_addr(&program_id),
        accounts: vec![
            solana_instruction::AccountMeta::new(to_addr(wallet), false),
            solana_instruction::AccountMeta::new_readonly(to_addr(&ore_mint), false),
            solana_instruction::AccountMeta::new(to_addr(&recipient_ata), false),
            solana_instruction::AccountMeta::new(to_addr(&stake), false),
            solana_instruction::AccountMeta::new(to_addr(&treasury), false),
            solana_instruction::AccountMeta::new(to_addr(&treasury_tokens), false),
            solana_instruction::AccountMeta::new(to_addr(&vesting), false),
            solana_instruction::AccountMeta::new_readonly(to_addr(&solana_sdk::system_program::id()), false),
            solana_instruction::AccountMeta::new_readonly(to_addr(&spl_token::id()), false),
            solana_instruction::AccountMeta::new_readonly(to_addr(&spl_associated_token_account::id()), false),
            solana_instruction::AccountMeta::new_readonly(to_addr(&program_id), false),
        ],
        data,
    }
}

pub fn run_claim() -> anyhow::Result<()> {
    let cfg = OreConfig::load();
    let kp_path = cfg.keypair_path();
    if !kp_path.exists() {
        anyhow::bail!("No keypair found at {}. Run `ore init` first.", kp_path.display());
    }

    let (pk, sk) = load_keypair(&kp_path)?;
    let identity = falcon_identity(&pk);
    let (vector_addr, _bump) = vector_pda(&pk);
    let wallet = Pubkey::new_from_array(vector_addr.to_bytes());

    println!("Claiming all yield...");
    let sub_ix = build_claim_ix(&wallet, u64::MAX);
    sign_and_send(&cfg, &identity, &sk, sub_ix)?;

    Ok(())
}

pub fn run_deposit(amount_str: &str) -> anyhow::Result<()> {
    let cfg = OreConfig::load();
    let kp_path = cfg.keypair_path();
    if !kp_path.exists() {
        anyhow::bail!("No keypair found at {}. Run `ore init` first.", kp_path.display());
    }

    let amount = parse_ore_amount(amount_str)?;
    if amount == 0 {
        anyhow::bail!("amount must be greater than 0");
    }

    let (pk, sk) = load_keypair(&kp_path)?;
    let identity = falcon_identity(&pk);
    let (vector_addr, _bump) = vector_pda(&pk);
    let wallet = Pubkey::new_from_array(vector_addr.to_bytes());

    let fee_payer = read_keypair_file(cfg.fee_payer_path())
        .map_err(|e| anyhow::anyhow!("failed to read fee payer keypair: {e}"))?;

    println!("Depositing {amount_str} ORE to stake...");
    let sub_ix = build_deposit_ix(&wallet, &fee_payer.pubkey(), amount, 0, 0);
    sign_and_send(&cfg, &identity, &sk, sub_ix)?;

    Ok(())
}

pub fn run_balance() -> anyhow::Result<()> {
    use ore_stake_api::prelude::*;
    use steel::AccountDeserialize;

    let cfg = OreConfig::load();
    let kp_path = cfg.keypair_path();
    if !kp_path.exists() {
        anyhow::bail!("No keypair found at {}. Run `ore init` first.", kp_path.display());
    }

    let (pk, _sk) = load_keypair(&kp_path)?;
    let (vector_addr, _bump) = vector_pda(&pk);
    let wallet = Pubkey::new_from_array(vector_addr.to_bytes());
    let stake_addr = ore_stake_api::state::stake_pda(wallet).0;

    let rpc = RpcClient::new(cfg.rpc_url());

    // Fetch stake account
    let stake_account = match rpc.get_account(&stake_addr) {
        Ok(a) => a,
        Err(_) => {
            println!("No stake account found. Use `ore stake deposit` to get started.");
            return Ok(());
        }
    };
    let mut stake = *Stake::try_from_bytes(&stake_account.data)?;

    // Fetch treasury, vesting, and clock for real-time reward calculation
    let treasury_addr = ore_stake_api::state::treasury_pda().0;
    let vesting_addr = ore_stake_api::state::vesting_pda().0;
    let treasury_account = rpc.get_account(&treasury_addr)?;
    let vesting_account = rpc.get_account(&vesting_addr)?;
    let clock_data = rpc.get_account_data(&solana_sdk::sysvar::clock::ID)?;

    let mut treasury = *Treasury::try_from_bytes(&treasury_account.data)?;
    let vesting = Vesting::try_from_bytes(&vesting_account.data)?;
    let clock: steel::Clock = bincode::deserialize(&clock_data)?;

    // Simulate update_rewards to get real-time yield
    let mut vesting = *vesting;
    stake.update_rewards(&clock, &mut treasury, &mut vesting);

    println!("Stake: {}", format_ore(stake.balance));
    println!("Yield: {}", format_ore(stake.rewards));

    Ok(())
}

pub fn run_withdraw(amount_str: &str) -> anyhow::Result<()> {
    let cfg = OreConfig::load();
    let kp_path = cfg.keypair_path();
    if !kp_path.exists() {
        anyhow::bail!("No keypair found at {}. Run `ore init` first.", kp_path.display());
    }

    let amount = parse_ore_amount(amount_str)?;
    if amount == 0 {
        anyhow::bail!("amount must be greater than 0");
    }

    let (pk, sk) = load_keypair(&kp_path)?;
    let identity = falcon_identity(&pk);
    let (vector_addr, _bump) = vector_pda(&pk);
    let wallet = Pubkey::new_from_array(vector_addr.to_bytes());

    println!("Withdrawing {amount_str} ORE from stake...");
    let sub_ix = build_withdraw_ix(&wallet, amount);
    sign_and_send(&cfg, &identity, &sk, sub_ix)?;

    Ok(())
}
