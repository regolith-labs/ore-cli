//! Vendored subset of `vector-core` — scheme, instructions, digest, and
//! Falcon-512 helpers. Avoids a path/git dependency so the crate can be
//! published to crates.io independently.

use sha2::{Digest as Sha2Digest, Sha256};
use solana_address::{address, Address};
use solana_instruction::{AccountMeta, BorrowedAccountMeta, BorrowedInstruction, Instruction};
use solana_instructions_sysvar::construct_instructions_data;

// ── Constants ────────────────────────────────────────────────────────────────

const SYSTEM_PROGRAM_ID: Address = address!("11111111111111111111111111111111");
const INSTRUCTIONS_SYSVAR_ID: Address = address!("Sysvar1nstructions1111111111111111111111111");

const INITIALIZE_DISCRIMINATOR: u8 = 0;
const ADVANCE_DISCRIMINATOR: u8 = 1;
const PASSTHROUGH_DISCRIMINATOR: u8 = 4;

const VECTOR_PDA_SEED: &[u8] = b"vector";

// Falcon-512 wire sizes (from solana-falcon512)
pub const FALCON512_WIRE_PUBKEY_LEN: usize = 897;
pub const FALCON512_SIGNATURE_LEN: usize = 666;
const FALCON512_PREPARED_PUBKEY_LEN: usize = 1024;
const FALCON512_STORED_IDENTITY_LEN: usize = 32 + 1 + FALCON512_PREPARED_PUBKEY_LEN;

// ── Scheme ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scheme {
    pub program_id: Address,
    pub signature_len: usize,
    pub identity_len: usize,
    pub stored_identity_len: usize,
}

pub const FALCON512: Scheme = Scheme {
    program_id: address!("BoS6ho8tZU8iRLNi9VR8dYXihrsyNUMU7nvgiAChPesU"),
    signature_len: FALCON512_SIGNATURE_LEN,
    identity_len: 32,
    stored_identity_len: FALCON512_STORED_IDENTITY_LEN,
};

// ── VectorAccount ────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VectorAccount {
    pub nonce: [u8; 32],
    pub bump: u8,
}

impl VectorAccount {
    pub const HEADER_LEN: usize = 33;

    pub fn from_header_bytes(bytes: &[u8; Self::HEADER_LEN]) -> Self {
        let mut nonce = [0u8; 32];
        nonce.copy_from_slice(&bytes[..32]);
        VectorAccount {
            nonce,
            bump: bytes[32],
        }
    }
}

// ── PDA derivation ───────────────────────────────────────────────────────────

fn pda_seed_from_identity(identity: &[u8]) -> [u8; 32] {
    if identity.len() <= 32 {
        let mut out = [0u8; 32];
        out[..identity.len()].copy_from_slice(identity);
        out
    } else {
        Sha256::digest(identity).into()
    }
}

pub fn find_vector_pda(scheme: &Scheme, identity: &[u8]) -> (Address, u8) {
    let seed_bytes = pda_seed_from_identity(identity);
    let seed_len = identity.len().min(32);
    Address::find_program_address(
        &[VECTOR_PDA_SEED, &seed_bytes[..seed_len]],
        &scheme.program_id,
    )
}

// ── Falcon-512 helpers ───────────────────────────────────────────────────────

pub fn falcon512_identity(wire_pubkey: &[u8; FALCON512_WIRE_PUBKEY_LEN]) -> [u8; 32] {
    Sha256::digest(wire_pubkey).into()
}

pub fn create_initialize_falcon512(
    payer: &Address,
    wire_pubkey: &[u8; FALCON512_WIRE_PUBKEY_LEN],
) -> Instruction {
    let identity = falcon512_identity(wire_pubkey);
    create_initialize_instruction(payer, &FALCON512, &identity, wire_pubkey)
}

// ── Instruction builders ─────────────────────────────────────────────────────

fn create_initialize_instruction(
    payer: &Address,
    scheme: &Scheme,
    identity: &[u8],
    init_payload: &[u8],
) -> Instruction {
    let (vector, _bump) = find_vector_pda(scheme, identity);
    let mut data = Vec::with_capacity(1 + init_payload.len());
    data.push(INITIALIZE_DISCRIMINATOR);
    data.extend_from_slice(init_payload);
    Instruction {
        program_id: scheme.program_id,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(vector, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data,
    }
}

pub fn create_advance_instruction(
    scheme: &Scheme,
    identity: &[u8],
    advance_vector_signature: &[u8],
) -> Instruction {
    let (vector_pda, _bump) = find_vector_pda(scheme, identity);
    let mut data = Vec::with_capacity(1 + advance_vector_signature.len());
    data.push(ADVANCE_DISCRIMINATOR);
    data.extend_from_slice(advance_vector_signature);
    Instruction {
        program_id: scheme.program_id,
        accounts: vec![
            AccountMeta::new(vector_pda, false),
            AccountMeta::new_readonly(INSTRUCTIONS_SYSVAR_ID, false),
        ],
        data,
    }
}

pub fn create_passthrough_instruction(
    scheme: &Scheme,
    identity: &[u8],
    instructions: &[Instruction],
) -> Instruction {
    let (vector_pda, _bump) = find_vector_pda(scheme, identity);

    let flattened_accounts: usize = instructions.iter().map(|ix| 1 + ix.accounts.len()).sum();
    let mut accounts = Vec::with_capacity(2 + flattened_accounts);
    accounts.push(AccountMeta::new(vector_pda, false));
    accounts.push(AccountMeta::new_readonly(INSTRUCTIONS_SYSVAR_ID, false));
    for ix in instructions {
        accounts.push(AccountMeta::new_readonly(ix.program_id, false));
        accounts.extend(ix.accounts.iter().cloned());
    }

    let payload_len: usize = 1 + 1 + instructions.iter().map(|ix| 1 + 2 + ix.data.len()).sum::<usize>();
    let mut data = Vec::with_capacity(payload_len);
    data.push(PASSTHROUGH_DISCRIMINATOR);
    data.push(instructions.len() as u8);
    for ix in instructions {
        data.push(ix.accounts.len() as u8);
        data.extend_from_slice(&(ix.data.len() as u16).to_le_bytes());
        data.extend_from_slice(&ix.data);
    }

    Instruction {
        program_id: scheme.program_id,
        accounts,
        data,
    }
}

// ── Digest ───────────────────────────────────────────────────────────────────

pub fn advance_vector_digest(
    scheme: &Scheme,
    nonce: &[u8; 32],
    identity: &[u8],
    pre_instructions: &[Instruction],
    post_instructions: &[Instruction],
) -> [u8; 32] {
    let sig_len = scheme.signature_len;
    let placeholder = vec![0u8; sig_len];
    let advance_ix = create_advance_instruction(scheme, identity, &placeholder);

    let mut all_owned: Vec<Instruction> =
        Vec::with_capacity(pre_instructions.len() + 1 + post_instructions.len());
    all_owned.extend(pre_instructions.iter().cloned());
    let advance_index = all_owned.len();
    all_owned.push(advance_ix);
    all_owned.extend(post_instructions.iter().cloned());

    let borrowed_ixs: Vec<BorrowedInstruction> = all_owned
        .iter()
        .map(|ix| BorrowedInstruction {
            program_id: &ix.program_id,
            accounts: ix
                .accounts
                .iter()
                .map(|m| BorrowedAccountMeta {
                    pubkey: &m.pubkey,
                    is_signer: m.is_signer,
                    is_writable: m.is_writable,
                })
                .collect(),
            data: &ix.data,
        })
        .collect();
    let buffer = construct_instructions_data(&borrowed_ixs);

    let ix_offset_pos = 2 + 2 * advance_index;
    let ix_offset = u16::from_le_bytes(
        buffer[ix_offset_pos..ix_offset_pos + 2].try_into().unwrap(),
    ) as usize;

    let num_accounts = all_owned[advance_index].accounts.len();
    let sig_start = ix_offset + 2 + 33 * num_accounts + 32 + 2 + 1;
    let sig_end = sig_start + sig_len;

    let mut hasher = Sha256::new();
    hasher.update(&buffer[..sig_start]);
    hasher.update(nonce);
    hasher.update(identity);
    hasher.update(&buffer[sig_end..]);
    hasher.finalize().into()
}
