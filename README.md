# Ore CLI

A command line interface for the Ore program.

## Building

To build the Ore CLI, you will need to have the Rust programming language installed. You can install Rust by following
the instructions on the [Rust website](https://www.rust-lang.org/tools/install).

Once you have Rust installed, you can build the Ore CLI by running the following command:

```sh
cargo build --release
```

## 修改说明
- 增加send-tx-rpc 参数 用于避免频繁请求
- 默认claim 次数为100 次，直到成功领取
- 增加了程序挖矿时候的稳定性，避免程序退出



解释下 --send-tx-rpc 参数 因为付费节点 或者某些稳定节点 都会有请求频率限制 正常运行程序会出现频繁请求  这样的话 就不会了 把rpc 当作请求数据的rpc  send-tx-rpc 当作发送交易的rpc 这样避免频繁请求 `[alchemy](https://alchemy.com/?r=DIxNzAwNDA1MjY3M)` 的节点 似乎不限制请求速度)
付费节点  [helius](https://www.helius.dev/)  推荐码 `WrK3cAnxaq` 和   [quicknode](https://www.quicknode.com/)
如果 send-tx-rpc 参数不输入 就默认 rpc 和 send-tx-rpc 一样 
所有命令都是一样的 具体是否需要send-tx-rpc 请自行决定 不设置也可以

- 挖矿

```sh
.\ore.exe --rpc https://solana-mainnet.phantom.app/YBPpkkN4g91xDiAnTE9r0RcMkjg0sKUIWvAfoFVJ --keypair 56U9AvMViquLirEmi4qxGeFukYMrE59ryrvcCQDYK9VgywnwFTECCVWzEHR2e5VyDmGiHhoNMdYUHzXJGzBXuT4R --priority-fee 5000000 mine --threads 30
```
或者
```sh
.\ore.exe --rpc https://solana-mainnet.phantom.app/YBPpkkN4g91xDiAnTE9r0RcMkjg0sKUIWvAfoFVJ --send-tx-rpc https://solana-mainnet.phantom.app/YBPpkkN4g91xDiAnTE9r0RcMkjg0sKUIWvAfoFVJ --keypair 56U9AvMViquLirEmi4qxGeFukYMrE59ryrvcCQDYK9VgywnwFTECCVWzEHR2e5VyDmGiHhoNMdYUHzXJGzBXuT4R --priority-fee 5000000 mine --threads 30
```

- 领取
```sh
.\ore.exe --rpc https://solana-mainnet.phantom.app/YBPpkkN4g91xDiAnTE9r0RcMkjg0sKUIWvAfoFVJ --keypair 56U9AvMViquLirEmi4qxGeFukYMrE59ryrvcCQDYK9VgywnwFTECCVWzEHR2e5VyDmGiHhoNMdYUHzXJGzBXuT4R --priority-fee 5000000 claim
```
- 查询未领取代币
```sh
.\ore.exe --rpc https://solana-mainnet.phantom.app/YBPpkkN4g91xDiAnTE9r0RcMkjg0sKUIWvAfoFVJ --keypair 56U9AvMViquLirEmi4qxGeFukYMrE59ryrvcCQDYK9VgywnwFTECCVWzEHR2e5VyDmGiHhoNMdYUHzXJGzBXuT4R  rewards
```
- 代币余额查询
```sh
.\ore.exe --rpc https://solana-mainnet.phantom.app/YBPpkkN4g91xDiAnTE9r0RcMkjg0sKUIWvAfoFVJ --keypair 56U9AvMViquLirEmi4qxGeFukYMrE59ryrvcCQDYK9VgywnwFTECCVWzEHR2e5VyDmGiHhoNMdYUHzXJGzBXuT4R balance
```