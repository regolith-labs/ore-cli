# Ore CLI
A command line interface for the Ore program.

## Building the utility
To build the Ore CLI, you will need to have the Rust programming language installed. You can install Rust by following the instructions on the [Rust website](https://www.rust-lang.org/tools/install).

These instructions are for using a linux environment but also work on WSL2 on windows. I suspect they will work on most MAC computers as well.

Once you have Rust installed, you can build the Ore CLI by running the following command in the ore-cli folder:
```sh
./build_and_mine.sh nomine
```
The first build can be slow so please be patient while each library is compiled. Subsequent rebuilds will be significantly quicker.
If the compilation fails, errors will be shown on screen for you to rectify.

The build process creates a commpiled ore cli in the path ```./target/release/ore```. This is the ore cli utility

## Rebuilding the utility
Save your edits to the source code then execute ```./build_and_mine.sh```. If the build is successful, a mining session will automatically be started.

## Configure your RPC
For privacy, you need to create a new file in the root of the ore-cli folder. This should be called ```ore_env.priv.sh```

An exampl of this file is:
```sh
COINGECKO_APIKEY=CG-XXXXXXXXXXXXXXXXXXXXX
KEY1=~/.config/solana/wallet_devnet_test1.json
RPC1=https://api.devnet.solana.com
PRIORITY_FEE_0=0
```

You should enter your rpc url into this file as RPC1 which is the default used withing the other scripts. The public RPC's should work but I have generally
found them to be quite unreliable for Ore mining.

## Setting up a wallet
Each miner needs a wallet to mine to. For testing purposes, you can create a new wallet for use with ore-cli.
```sh
./createwallet.sh ~/.config/solana/wallet_devnet_test1.json
```
This will lead you through creating a keypair file called whatever you like. It does not have to match the above example. Remember and store your seed phrase in case you need to recreate it at a later date or import it into some other solana wallet app.

Once you have the keypair file, you need to ensure it is set to the KEY1 variable in ```ore_env.prv.sh```. This is used as the default wallet in the scripts presented alongside ore-cli.

## Manually starting a mining session
Execute the command:
```sh
./miner.sh
```
This will start up a miner process that will use the wallet & RPC details configured in the ore_env.sh file. You should see the miner start up and see it mine its first hash. After 1 minute, you should get a transaction and a completed log message
```sh
----------------------------------------------------------------------------------------------------
Starting Miner 1
----------------------------------------------------------------------------------------------------
Wallet: /home/paul/.config/solana/id.json
RPC: https://XXXXXXXXXXXXXXXXXXXXXXXXXXX
Priority fee: 0
ore-cli: ./target/release/ore
----------------------------------------------------------------------------------------------------
Initial pass cutoff time: 52s

Pass 1 started at 22:30:07 on 2024-05-18        Mined for 0s    CPU: 3.05/3.26/3.31
        Currently staked ORE: 32.59427932637    Wallet SOL:  5.210645165        Last Withdrawal: 21.4 hours ago Withdrawal Penalty for 72 mins
  [53s] Difficulty: 13            Hash: 	xHo32mQX3j7GWHaAaMYTLh13aDyCZD9Re54Ahaji
  [1s] (attempt 2) SUCCESS        Txid: 4iUR6qQw4MXQ5sXzHWhgCHLmfQWPrHEsYbuf9FiCmxt3hTfMjEgHyPbckXhBzWNXcCJfdD8sQ87HYpCURAZ6hnT7
  [55s] Completed    - Mined: 0.00142957542           Cost: -0.000005000        Session: 0.00142957542 ORE      0.000005000 SOL
```

Congratulations, you have mined your first ORE.

The miner will keep looping until your wallet runs out of SOL. After each pass, any SOL mined is added to you staked ORE which will increase your
earnings from subsequent mining passes.

The difficulty of the hash your miner has resolved will determine how much ORE is rewarded to all miners that submit a hash at that difficulty level.
You will receive your share of the total for that difficulty. A higher difficulty level solved will get you a higher amount of ORE rewarded.
The amount you receive is variable each time so you will not usually get the same amount each time you solve the same difficulty level. There is a
highly complex algorithm that calculates this but you will need an enormous brain to understand how it is computed and if you think it is wrong then
tough luck as that is what you are getting whether you like it or not.

The miner will keep track of your Session Total for ORE mined and SOL spent.

At regular intervals, you will get a summary of how many hashes this miner has mined at each difficulty level. This will indicate how powerful the
hardware you are using to mine is. Comparing this between different computers may lead you to mine on your fastest or your most efficient. Note that
the same hardware may get a range of difficulties returned. Sometimes you get lucky and solve a complex one and get a better reward!
```sh
========================================================================================================================
| Difficulties solved in 10 passes:
|----|----|----|----|----|----|
|  12|  13|  14|  15|  16|  19|
|   1|   4|   2|   1|   1|   1|
========================================================================================================================
```

## Withdrawing Staked ORE
TO DO

## Staking Additional ORE
TO DO

## Close Accounts
TO DO - I have no idea what the purpose of this is yet.