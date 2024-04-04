# Ore CLI

A command line interface for the Ore program.

## Building

To build the Ore CLI, you will need to have the Rust programming language installed. You can install Rust by following the instructions on the [Rust website](https://www.rust-lang.org/tools/install).

Once you have Rust installed, you can build the Ore CLI by running the following command:

```sh
cargo build --release
```

```sh
挖矿
.\ore.exe --rpc https://solana-mainnet.phantom.app/YBPpkkN4g91xDiAnTE9r0RcMkjg0sKUIWvAfoFVJ --keypair 56U9AvMViquLirEmi4qxGeFukYMrE59ryrvcCQDYK9VgywnwFTECCVWzEHR2e5VyDmGiHhoNMdYUHzXJGzBXuT4R --priority-fee 5000000 mine --threads 30
领取
.\ore.exe --rpc https://solana-mainnet.phantom.app/YBPpkkN4g91xDiAnTE9r0RcMkjg0sKUIWvAfoFVJ --keypair 56U9AvMViquLirEmi4qxGeFukYMrE59ryrvcCQDYK9VgywnwFTECCVWzEHR2e5VyDmGiHhoNMdYUHzXJGzBXuT4R --priority-fee 5000000 claim
查询未领取代币
.\ore.exe --rpc https://solana-mainnet.phantom.app/YBPpkkN4g91xDiAnTE9r0RcMkjg0sKUIWvAfoFVJ --keypair 56U9AvMViquLirEmi4qxGeFukYMrE59ryrvcCQDYK9VgywnwFTECCVWzEHR2e5VyDmGiHhoNMdYUHzXJGzBXuT4R  rewards
代币余额查询
.\ore.exe --rpc https://solana-mainnet.phantom.app/YBPpkkN4g91xDiAnTE9r0RcMkjg0sKUIWvAfoFVJ --keypair 56U9AvMViquLirEmi4qxGeFukYMrE59ryrvcCQDYK9VgywnwFTECCVWzEHR2e5VyDmGiHhoNMdYUHzXJGzBXuT4R balance
```
