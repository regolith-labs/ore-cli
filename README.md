# Ore CLI

A command line interface for the Ore program.

## Commands

Commands are executed by running `ore <command> [options]`.

### `ore balance <address>`

Get the balance of an account.

#### Arguments

- `<address>`: The address of the account to get the balance of.

#### Example

```bash
ore \
--rpc https://api.mainnet-beta.solana.com \
--keypair ~/.config/solana/id.json \
balance \
kPcRLwwk1Qu3BizQcrCdGSzo6BjowqJNCWdDurgva7g
```