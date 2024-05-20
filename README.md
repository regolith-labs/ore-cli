# Ore CLI
A command line interface for the Ore program.

## Cloning the repositories
You will need to have git installed on your computer to clone, build and test this software. On debian/ubuntu this can usualy be done with ```sudo apt install git```. If this does not work, please google how to install git on your OS.

First create a suitable folder to clone the 3 git repositories to:
```sh
mkdir ~/ore2; cd ~/ore2
git clone https://github.com/hardhatchad/ore
git clone https://github.com/hardhatchad/ore-cli
git clone https://github.com/hardhatchad/drillx
cd ~/ore2/ore && git checkout hardhat/v2
cd ~/ore2/ore-cli && git checkout hardhat/v2
```
Execute each command separately one after the other watching for errors on the way.


## Building the utility
To build the Ore CLI, you will need to have the Rust programming language installed. You can install Rust by following the instructions on the [Rust website](https://www.rust-lang.org/tools/install).

The instructions presented here are for using a linux environment but also work on WSL2 on windows (I suspect they will work on most MAC computers as well).

Once you have Rust installed, you can build the Ore CLI by running the following command in the ore-cli folder:
```sh
cd ~/ore2/ore-cli
./build_and_mine.sh
```
The first build can be slow so please be patient while each library is compiled. Subsequent rebuilds will be significantly quicker. If the compilation fails, errors will be shown on screen for you to rectify.

The build process creates a compiled ore cli executable in the path ```./target/release/ore``` as well as a link to it in ```./ore```. This is the ore cli utility that you have compiled.

## Rebuilding & debugging the ore-cli utility
Save your edits to the source code then execute ```./build_and_mine.sh 1```. If the build is successful, a mining session will automatically be started for the first miner configured in ```ore_env.priv.sh```. Obviously, you need to follow the rest of the instructions here before attempting to do this as it does not know anything about your miner configuration yet.

## Setup your miner configuration
The scripts provided here all reference a file in the root of the ore-cli folder called ```ore_env.priv.sh```. This allows you to centralise your miner configuration and allows you to easily run as many miners as you have hardware to run them on and also to manage the wallets of your miners.

This file is excluded from the git repository as it contains personal information about your RPC URL, wallet locations, and a few other items about your miner configuration.

An example of this file is included in ```ore_env.priv.sh.sample``` and you can copy or rename this file to ```ore_env.priv.sh``` to get started. It has some comments in it that are probably worth reading.

You will need to configure at least 1 miner in this script to allow the other scripts in this application to work properly.

For each miner you need to specify RPC1, KEY1, THREADS1, PRIORITY_FEE1 and optionally MINER_WATTAGE_1.

A public RPC URL should work but I have generally found them to be quite unreliable for ORE mining. It is best to sign up for your own personal solana RPC endpoint from one of the providers such as QuickNode, Helius or any of the others.

A key file can be setup as described in the section ```Setting up a wallet```.

Threads should be set to a value less that or equal to the number of cores in your computer. Personally, I leave at least one thread free so the operating system can find time to respond whilst mining. eg. if you have 4 cores in your CPU then set threads to 3. This will lower your hashing power but means the computer does not grind to a halt for doing any other task whilst mining.

There are 2 other settings:
COINGECKO_APIKEY: This will be used to lookup the ORE & SOL price from coingecko to convert the value of your wallet into dollars.
ELECTRICITY_COST_PER_KILOWATT_HOUR: This will be used to calculate the cost of electricity for each miner if the have a MINER_WATTAGE setting specified.

## Setting up a wallet
Each miner requires a unique wallet to mine to because of the staking mechanism. It is pointless to mine the same wallet on multiple miners. You can create a new wallet for use with ore-cli using the script below. Note that devnet wallet are not interchangeable with mainnet wallets and your RPC URL dictates what network the new wallet will be valid on.
```sh
./createwallet.sh ~/.config/solana/wallet_devnet_test1.json
```
Note that this script will use the RPC1 URL defined in your configuration. This will lead you through creating a keypair file. It can be called whatever you like as long as you know where you create it and what it is called. It is best to keep these outside of the ore-cli folder so that it cannot accidentally be uploaded to git.

Remember and store your seed phrase in case you need to recreate it at a later date or import it into some other solana wallet app.

Once you have created the keypair file, you need to ensure the pathname is added as the KEY1 variable in ```ore_env.priv.sh```. This will be the wallet associated with miner 1.

## Funding your mining wallet
ORE mining is free. Your only charge for mining is the SOL transaction fees to submit your hashes each minute and also for staking/withdrawing your mined ORE.
Oh, and also your electric bill - you are taxing your computer harder than normal so it will be HOT, NOISY and cost more than normal to have powered on when mining.

You will need to transfer SOL into your mining wallet. Documenting this step is outwith the remit of this document but a pointer is to use something like the
Phantom Wallet browser plugin to transfer SOL from your main Solana wallet to your mining wallet.

If you are testing on devnet then you can airdrop yourself some SOL for free. The ```createwallet.sh``` script above will show you an exact command to do this
customised for your new wallet keypair file. It will be something like:
```sh
./airdropDevnetSol.sh 1 1.5
```
The command above will attempt to airdrop 1.5 SOL to miner 1's wallet. Be aware that your RPC will usually rate limit this and limit the actual amount you can airdrop and how oftem you can do it.

ORE uses very little SOL every minute and it will cost around 0.000005 SOL * 60 mins * 24 hours = 0.0072 SOL for an entire day's mining. If 1 SOL costs \$200 then that is about \$1.44 per day per miner.

This calculation is assuming your transaction priority fee is 0. If you are submitting with a crazy high number then your costs can skyrocket quickly for each transaction but you should not need to use an higher number unless the Solana network is heavily congested.

## Manually starting a mining session
Execute the command:
```sh
./miner.sh 1
```
This will start up a miner process that will use the first wallet & RPC URL configured in the ```ore_env.priv.sh``` file. You will see the miner start up and watch it mine its first hash. After about 1 minute, you should get a transaction and a completed log message:
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

Congratulations, you have mined your first ORE. Large wallets start with humble rewards...

The miner will keep looping indefinitely until your wallet runs out of SOL. After each pass, any ORE mined is added to the wallet's staked ORE. The amount you have staked increases your earnings in subsequent mining passes.

If you have run out of SOL, the miner will pause for 1 minute then check again to see if you have deposited more SOL. Once SOL is added, the miner will automatically start mining again. If not, it will wait indefinitely unitl SOL is available or you kill the miner process.

The difficulty of the hash your miner has resolved will determine how much ORE is rewarded to all miners that submit a hash at that difficulty level. You will receive your share of the total rewards for that difficulty. A higher difficulty level solved will get you a higher amount of ORE rewarded.

The amount you receive is variable each pass so you will not usually get the same amount each time you solve the same difficulty level. There is a
highly complex algorithm that calculates this but you will need an enormous brain to understand how it is computed and if you think it is wrong then
tough luck as that is what you are getting rewarded whether you like it or not.

The miner will keep track of your Session Totals for ORE mined and SOL spent.

At regular intervals, you will get a summary of how many hashes this miner has mined at each difficulty level.
```sh
========================================================================================================================
| Current ORE Price: $279.60    Current SOL Price: $171.02
| Max session reward: 0.00423792143 ORE ($1.1849) at difficulty 14 during pass 3.
| Average reward:     0.00273952412 ORE ($0.7660) over 5 passes.
| Session Summary:      Profit: $3.8299 ORE           Cost: $0.0043 SOL Profitablility: $3.8256
| Difficulties solved during 5 passes:
|----|----|----|----|
|  11|  13|  14|  17|
|   1|   2|   1|   1|
========================================================================================================================
```
You are shown the current ORE and SOL prices in dollars if you have setup the coingecko api key in your config.

You are presented with your maximum session reward so far and how much that is worth in dollars.

You are also shown your average amount of ORE earned per mining pass.

It then summarises your profit & costs for the session.

The difficulty table indicates approximately how powerful the hardware you are using run this miner on. Note that the same hardware may get a range of difficulties returned. Sometimes you get lucky and solve a more complex one in the 1 minute allowed and get a better reward! Comparing this data between different computers may lead you to mine on your fastest or your most efficient. It's up to you to decide.

You can stop the miner at any time without losing any rewards. On most computers this can be accomplished by pressing CTRL+C in the terminal where the miner is running. The next time you start your miner with the same wallet you will see that your staked sol is preserved between mining session.

## Checking your Wallet Balance
You do not need to have a mining session running to see the wallet balances. You can check on the state of a wallet at any time by:
```sh
./walletBalance.sh 1
```
This will show the amount of unstaked and staked ORE for the particular miner number as specified in your ```ore_env.priv.sh```.
In the above example this would use the key specified by KEY1 for miner 1.

## Staking Additional ORE
If you have unstaked ORE stored in your wallet then you can opt to stake it to increase your rewards multiplier when mining with that key file.
You can add staked ore at any time (even whilst mining). To stake ORE, execute the following command:
```sh
./stakeOre.sh 1 all
./stakeOre.sh 1 2
```
The first example will stake ALL ore in wallet 1.
The second example will stake an additional 2 ORE in wallet 1


## Withdrawing Staked ORE
==Please be careful when staking ore - there is a penalty if you unstake it within 24 hours. You could lose part of your staked ORE if you withdraw too early. After 24 hours you will get the entire amount unstaked.==

You can withdraw your staked ORE at any point and move it to your wallet as ORE. This can the be transferred to another wallet or converted to another token (eg. to USDC or SOL).
```sh
./withdrawStakedOre.sh 1 all
./withdrawStakedOre.sh 1 15
```
Example 1 will unstaked all your ORE for wallet 1.
Example 2 will unstake 15 ORE from wallet 1 (if it has 15 ORE or more staked)

If you are trying to unstake too soon after mining or manually staking ORE then you will receive a warning and be told how much you ORE will permanently lose. You can opt out at this point which is nice.
```sh
paul@paulsExtWin10:~/ore2/ore-cli$ ./withdrawStakedOre.sh 2 0.00189869703
20240519223218 wallet_devnet2.json Wallet 2 ORE balance: 0.00000000000 ORE ($0.00)      Staked: 0.10189869705 ORE ($28.77)
This wallet can currently withdraw up to 0.10189869705 staked ORE worth $28.77.
Your rewards of $28.77 are greater than $0.10 so proceeding to claim rewards.
----------------------------------------------------------------------------------------------------------

WARNING You are about to burn 0.00188107255 ORE!
Claiming more frequently than once per day is subject to a burn penalty.
Your last claim was 0.21 hours ago. You must wait 1426 minutes to avoid this penalty.

Are you sure you want to continue? [Y/n]
y
  [1s] (attempt 3) SUCCESS        Txid: 5mAbYMFNYET7k3PUY2SF6joJPip5MQ6DKEQgaDUEo4DBvGcnp8dtcriAQtvAocdxB3ixtt8T16ff4Woq7TgV1NR5                                                                                ==========================================================================================================
The wallet balance after withdrawing the staked ore is:
20240519223309 wallet_devnet2.json Wallet 2 ORE balance: 0.00001874524 ORE ($0.01)      Staked: 0.10000000002 ORE ($28.23)
```

## Close Accounts
TO DO - I have no idea what the purpose of this is yet.
