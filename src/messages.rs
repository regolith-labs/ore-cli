use solana_program::pubkey::Pubkey;
use solana_sdk::{keccak::Hash, signature::Keypair, signer::Signer};

use crate::utils::proof_address;

pub struct MineJob {
    pub tunnel: Tunnel,
    pub challenge: Hash,
    pub difficulty: Hash,
    pub total_hashes: u64,
}

pub struct SendJob {
    pub tunnel: Tunnel,
    pub hash: Hash,
    pub nonce: u64,
    pub total_hashes: u64,
}

pub struct Tunnel {
    pub id: usize,
    pub proof: Pubkey,
    pub keypair: Box<Keypair>,
}

impl Tunnel {
    pub fn new(keypair: Keypair, id: usize) -> Tunnel {
        Tunnel {
            id,
            proof: proof_address(keypair.pubkey()),
            keypair: Box::new(keypair),
        }
    }
}
