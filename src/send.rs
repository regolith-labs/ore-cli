use pqcrypto_falcon::falcon512;
use pqcrypto_traits::sign::DetachedSignature;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::Signer,
    signer::keypair::read_keypair_file,
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;
use crate::vector::{
    advance_vector_digest, create_advance_instruction, create_passthrough_instruction,
    FALCON512, FALCON512_SIGNATURE_LEN, VectorAccount,
};

use crate::config::OreConfig;
use crate::init::to_sdk_instruction;
use crate::keypair::{load_keypair, falcon_identity, vector_pda};

const ORE_MINT: &str = "oreoU2P8bN6jkk3jbaiVxYnG1dCXcYxwhwyK9jSybcp";
const ORE_DECIMALS: u8 = 11;

pub fn run(amount_str: &str, recipient_str: &str) -> anyhow::Result<()> {
    let cfg = OreConfig::load();
    let kp_path = cfg.keypair_path();

    if !kp_path.exists() {
        anyhow::bail!("No keypair found at {}. Run `ore init` first.", kp_path.display());
    }

    // Parse amount
    let amount = parse_ore_amount(amount_str)?;
    if amount == 0 {
        anyhow::bail!("amount must be greater than 0");
    }

    // Parse recipient
    let recipient = recipient_str.parse::<Pubkey>()?;

    // Load keys
    let (pk, sk) = load_keypair(&kp_path)?;
    let identity = falcon_identity(&pk);
    let (vector_addr, _bump) = vector_pda(&pk);
    let vector_pubkey = Pubkey::new_from_array(vector_addr.to_bytes());
    let ore_mint = ORE_MINT.parse::<Pubkey>()?;
    let source_ata = get_associated_token_address(&vector_pubkey, &ore_mint);

    // The recipient could be a wallet address or a token account.
    // We assume it's a token account. If the user passes a wallet address,
    // they should use the ATA of that wallet for the ORE mint.
    let dest_ata = if is_token_account_address(recipient_str) {
        recipient
    } else {
        // Assume it's a wallet address; derive its ATA
        get_associated_token_address(&recipient, &ore_mint)
    };

    // Load fee payer
    let fee_payer = read_keypair_file(cfg.fee_payer_path())
        .map_err(|e| anyhow::anyhow!("failed to read fee payer keypair: {e}"))?;
    let rpc = RpcClient::new(cfg.rpc_url());

    // Fetch current nonce from the Vector account
    let vector_account_data = rpc.get_account_data(&vector_pubkey)?;
    if vector_account_data.len() < VectorAccount::HEADER_LEN {
        anyhow::bail!("Smart wallet not initialized — run `ore init` first.");
    }
    let header: [u8; VectorAccount::HEADER_LEN] =
        vector_account_data[..VectorAccount::HEADER_LEN].try_into()?;
    let vector_account = VectorAccount::from_header_bytes(&header);
    let nonce = vector_account.nonce;

    // Build the SPL Token transfer_checked instruction (as a solana_instruction::Instruction
    // for vector-core, since passthrough takes that type).
    let spl_transfer_ix = spl_transfer_checked_instruction(
        &source_ata,
        &ore_mint,
        &dest_ata,
        &vector_pubkey,
        amount,
        ORE_DECIMALS,
    );

    // Build the passthrough instruction (wraps the SPL transfer under the Vector PDA's authority)
    let passthrough_ix =
        create_passthrough_instruction(&FALCON512, &identity, &[spl_transfer_ix]);

    // Compute the advance digest.
    // Transaction layout: [advance, passthrough]
    // advance is at index 0, passthrough is post.
    let digest = advance_vector_digest(
        &FALCON512,
        &nonce,
        &identity,
        &[],                                         // no pre-instructions
        std::slice::from_ref(&passthrough_ix),       // passthrough is post
    );

    // Sign with Falcon-512
    let raw_sig = falcon512::detached_sign(&digest, &sk);
    let sig_bytes = raw_sig.as_bytes();
    let mut signature = [0u8; FALCON512_SIGNATURE_LEN];
    if sig_bytes.len() > FALCON512_SIGNATURE_LEN {
        anyhow::bail!("Falcon-512 signature too long");
    }
    signature[..sig_bytes.len()].copy_from_slice(sig_bytes);

    // Build the advance instruction
    let advance_ix = create_advance_instruction(&FALCON512, &identity, &signature);

    // Convert to solana-sdk instructions
    let sdk_advance = to_sdk_instruction(advance_ix);
    let sdk_passthrough = to_sdk_instruction(passthrough_ix);

    // Assemble and send transaction
    let recent_blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[sdk_advance, sdk_passthrough],
        Some(&fee_payer.pubkey()),
        &[&fee_payer],
        recent_blockhash,
    );

    crate::tx::send_and_confirm(&rpc, &tx, &format!("Sending {amount_str} ORE to {dest_ata}..."))?;

    Ok(())
}

/// Parse a human-readable ORE amount (e.g., "1.5") into base units (u64).
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

/// Build an SPL Token `transfer_checked` instruction as a `solana_instruction::Instruction`
/// (the type vector-core's passthrough builder expects).
fn spl_transfer_checked_instruction(
    source: &Pubkey,
    mint: &Pubkey,
    destination: &Pubkey,
    authority: &Pubkey,
    amount: u64,
    decimals: u8,
) -> solana_instruction::Instruction {
    // We build this manually to produce a solana_instruction::Instruction
    // rather than a solana_sdk::instruction::Instruction.
    let spl_token_id: [u8; 32] = spl_token::id().to_bytes();

    // transfer_checked instruction data: [12 (u8), amount (u64 LE), decimals (u8)]
    let mut data = Vec::with_capacity(10);
    data.push(12); // TransferChecked discriminator
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(decimals);

    solana_instruction::Instruction {
        program_id: solana_address::Address::new_from_array(spl_token_id),
        accounts: vec![
            solana_instruction::AccountMeta::new(
                solana_address::Address::new_from_array(source.to_bytes()),
                false,
            ),
            solana_instruction::AccountMeta::new_readonly(
                solana_address::Address::new_from_array(mint.to_bytes()),
                false,
            ),
            solana_instruction::AccountMeta::new(
                solana_address::Address::new_from_array(destination.to_bytes()),
                false,
            ),
            solana_instruction::AccountMeta::new(
                solana_address::Address::new_from_array(authority.to_bytes()),
                false, // not a tx-level signer — passthrough promotes the PDA via CPI
            ),
        ],
        data,
    }
}

/// Heuristic: we just treat the recipient as a wallet address and derive its ATA.
/// This is always correct for `send` — the user sends to a wallet, we derive the ATA.
fn is_token_account_address(_addr: &str) -> bool {
    false // always derive ATA from wallet address
}
