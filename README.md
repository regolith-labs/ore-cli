# ORE CLI

ORE quantum-safe smart wallet. Built on [Solana](https://solana.com) with [Vector](https://github.com/blueshift-gg/vector), using [Falcon-512](https://falcon-sign.info/) post-quantum signatures.

## How it works

ORE uses a smart wallets to protect your funds against quantum attacks. Instead of holding tokens in a standard Solana keypair, your ORE is stored in an on-chain smart wallet that requires **two signatures** to move funds:

1. **Ed25519** — A standard Solana signature from your fee-payer keypair. This pays for transaction fees and is verified by the Solana runtime as usual.
2. **Falcon-512** — A post-quantum signature verified by the Vector program on-chain. This is what actually authorizes transfers out of your smart wallet.

Even if an attacker could break Ed25519 with a quantum computer, they still cannot move your funds without also forging a Falcon-512 signature — which is designed to be resistant to quantum attacks. Your Falcon-512 private key never touches the Solana runtime and is only used locally to sign transaction digests.

## Install

```sh
cargo install ore-cli
```

## Setup

Initialize your config and set a custom RPC if needed:

```sh
ore config set rpc-url https://your-rpc-url.com
ore config set fee-payer ~/.config/solana/id.json
```

Initialize your smart wallet:

```sh
ore init
```

## Commands

| Command | Description |
|---------|-------------|
| `ore wallet` | Print wallet summary (address, balances, stake, signer) |
| `ore address` | Print smart wallet address |
| `ore signer` | Print signer address and SOL balance |
| `ore balance` | Show ORE token balance |
| `ore transfer <AMOUNT> <RECIPIENT>` | Transfer ORE to a recipient |
| `ore pay` | Display QR code for receiving ORE payments |
| `ore pay <AMOUNT>` | Display QR code with amount pre-filled |
| `ore stake deposit <AMOUNT>` | Deposit ORE into stake |
| `ore stake withdraw <AMOUNT>` | Withdraw ORE from stake |
| `ore stake balance` | Show staked balance and yield |
| `ore stake claim` | Claim all yield |
| `ore config` | View current configuration |
| `ore config set <KEY> <VALUE>` | Set a config value (`keypair`, `fee-payer`, `rpc-url`) |
| `ore init` | Initialize a new smart wallet |
