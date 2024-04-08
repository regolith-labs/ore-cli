#!/bin/bash

echo "Welcome to Ore Miner Setup."
echo "I'm going to run some checks to see if you have prerequisites installed."
echo ""; echo "Please read all instructions CAREFULLY. This is all important."; echo ""

echo "Testing if solana cli is installed."
if command -v solana > /dev/null 2>&1; then
	echo "solana cli installed, OK"
else
	echo "solana cli is required for ore mining. Would you like me to install it now? If no, program will exit"
	echo "Enter [y/n]"
	read install1
	if [ $install1 = "y" ]; then
		echo "Installing solana cli. You will see other logging messages from the solana cli being installed."
		sh -c "$(curl -sSfL https://release.solana.com/stable/install)"
	else 
		echo "Exiting program."
		exit 0
	fi
fi
export PATH="/home/$USER/.local/share/solana/install/active_release/bin:$PATH"
echo "Testing if rust is installed"
if command -v cargo > /dev/null 2>&1; then
	echo "Rust installed, OK"
else
	echo "Rust is required for ore mining. Would you like me to install it now? If no, program will exit"
	echo "Enter [y/n]"
	read install2
	if [ $install2 = "y" ]; then
		echo "You will see other logging messages from Rust being installed"
		curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
	else
		echo "Exiting program."
		exit 0
	fi
fi
echo "Testing if ore-cli is installed"
if command -v ore > /dev/null 2>&1; then
	echo "ore-cli installed, OK"
else
	echo "Now installing ore-cli. You will see other logging messages from ore-cli being installed."
	cargo install ore-cli
fi
NUM_PROCESSORS=$(nproc)

# REMOVE AFTER TESTING
NUM_PROCESSORS=4

NUM_MINERS=$((NUM_PROCESSORS - 2))

# TO DO: change this to mainnet-beta when ready to release
solana config set -u devnet > /dev/null

while true; do
	echo "Your PC has $NUM_PROCESSORS processors. It is recommended that you use 2 threads per CPU and mine on separate accounts"
	echo "and leave some space for other operations on the PC, so it will leave two processors free"; echo ""
	echo "To follow this recommendation, type a for accept."
	echo "To ignore this recommendation, type d for deny and provide 1) the number of accounts to generate and"
	echo "2) the number of threads to assign to each account"; echo ""
	read choice2
	if [ "$choice2" == "a" ]; then
		echo "You will have to either set a password for $NUM_MINERS keys, or just press enter a bunch to skip setting passwords"
		pubkey_list=()
		for ((i=1; i<=$NUM_MINERS; i++)) do
			solana-keygen new -o /home/user/.config/solana/id$i.json
			solana config set -k ~/.config/solana/id$i.json > /dev/null
			pubkey_list+=($(solana address))
		done
		break
	elif [ "$choice2" == "d" ]; then
		echo "OK, you need to enter the number of accounts you want"
		echo "Number of accounts: "
		read num_accounts
		echo "Enter the number of threads you want to use per account"
		read num_threads
		pubkey_list=()
		for ((i=1; i<=$num_accounts; i++)) do
			solana config set -k ~/.config/solana/id$i.json > /dev/null
			pubkey_list+=$(solana address)
		done
		break
	else 
		echo "Invalid choice."
	fi
done

#REMOVE AFTER TESTING
# This just resets the pubkey_list to the actual list since I'm not generating a real one during testing
pubkey_list=()
pubkey_list+=($(solana address -k ~/.config/solana/id1.json))
pubkey_list+=($(solana address -k ~/.config/solana/id2.json))

# TO DO: This doesn't account for alternate selections from above yet
echo ""; echo "I have generated $NUM_MINERS private keys labelled id#.json in the directroy ~/.config/solana/ for you."
echo "They will now be backed up by creating a tar file containing all of them, placed in your home directory."
echo "I ain't gonna tell you this again."
echo "*	*	*	*	*	*	*	*	*	*	*	*	*"
echo "Listen here, clown! Are you listening?"; echo ""
echo "IF YOU LOSE THIS FILE, YOU MAY LOSE ALL YOUR SOL AND YOUR ORE."; echo ""
echo "I realize this is a lot private keys to keep track of, but you need to do it. Put it on a thumb drive or something."
echo "So help me God, if you come crying to me you lost your keys, you will find no sympathy. Do you understand?"; echo ""
while true; do
	echo "Press y for 'I got it' and proceed. Press n to exit the program."
	echo "[y/n]"; echo ""
	read choice3
		
	if [ "$choice3" == "y" ]; then
		tar -cf keys.tar *
		mv keys.tar ~
		echo "Remember, I placed a file named keys.tar in your home director which is here: $(echo ~).  Don't screw this up."; echo ""
		break
	elif [ "$choice3" == "n" ]; then 
		echo "It's ok, you weren't ready to accept the responsibility."
		exit 0
	else 
		echo "Invalid choice."
	fi
done
MIN_SOL=$(echo "scale=5; $NUM_MINERS * 0.025" | bc)
echo "The next thing we need to do is fund one of the accounts to use to pay for fees. This program will then disburse the funds"
echo "to the rest of the accounts, so we need to get the amount right.  Each account should have a minimum of 0.025 SOL, or you'll"
echo "run out fairly quickly. You generated $NUM_MINERS keys.  That means you'll need $MIN_SOL SOL at a minimum."
solana config set -k ~/.config/solana/id1.json > /dev/null
MAIN_ACCOUNT=$(solana address)
while true; do
	echo "You need to fund this address $MAIN_ACCOUNT with $MIN_SOL, and then I will divide it into all the other accounts."
	echo "I will pause while you fund the initial account. Press c to continue once you have sent the necessary SOL to"
	echo "$MAIN_ACCOUNT and are ready to proceed.  Type exit to exit program."
	read choice4
	if [ "$choice4" = "c" ]; then
		echo "Checking balance on main account "
		MAIN_ACCT_BAL=$(solana balance | awk '{print $1}')
		if [ $(echo "$MAIN_ACCT_BAL == 0" | bc) -eq 1 ]; then
			echo "Main account balance is 0 still, try again."
		elif [ $(echo "$MAIN_ACCT_BAL > 0" | bc) -eq 1 ] && [ $(echo "scale=5; $MAIN_ACCT_BAL < $MIN_SOL" | bc) -eq 1 ]; then
			echo "You have $MAIN_ACCT_BAL SOL in your wallet, but this is less than the recommended minimum of $MIN_SOL. Send some more."
		elif [ $(echo "$MAIN_ACCT_BAL >= $MIN_SOL" | bc) -eq 1 ]; then
			echo "Ok, id1.json has $MAIN_ACCT_BAL which is greater than the minimum recommended $MIN_SOL SOL"
			SOL_PER_MINER=$(echo "scale=5; $MAIN_ACCT_BAL / $NUM_MINERS" | bc )
			break
		fi
	elif [ $choice4 == "exit" ]; then
		echo "Exiting program."
		exit 0
	else 
		echo "Invalid choice."
	fi
done
while true; do
	echo ""; echo "Now I will disburse the SOL in the main account $MAIN_ACCOUNT across the rest of the so they all have enough to pay fees."
	# Double check it's set to acct 1 that has the SOL in it
	solana config set -k ~/.config/solana/id1.json > /dev/null
	MAIN_ACCT_BAL=$(echo $(solana balance) | awk '{print $1}')
	AMT_TO_TRANSFER=$( echo "scale=5; $MAIN_ACCT_BAL / $NUM_MINERS" | bc )
	echo "Your main account now has $MAIN_ACCT_BAL SOL.  I'm going to divide that equally across all miner accounts so they all contain"
	echo "about $AMT_TO_TRANSFER SOL.  Some of the transactions may fail, but will be retried automatically."
	echo "Press y to confirm you want to distribute the SOL from your main account across the other $NUM_MINERS miner accounts or"
	echo "type exit to exit."; echo ""
	read choice5

	if [ "$choice5" = "y" ]; then
		echo "Beginning transfers. This will take some time as they finalize, and especially if we need multiple retries."
		for ((i=1; i<${#pubkey_list[@]}; i++)) do
			solana transfer ${pubkey_list[i]} $AMT_TO_TRANSFER --allow-unfunded-recipient
		done
		echo "Checking that all accounts now have SOL for fees"
		empty_accts=()
		SUCCESS_FLAG=false
		for ((i=0; i<${#pubkey_list[@]}; i++)) do 
			TEMP_BAL=$(solana balance ${pubkey_list[i]} | awk '{print $1}')
			if [ $(echo "$TEMP_BAL > 0" | bc) -eq 0 ]; then
				echo "${pubkey_list[i]} has a 0 balance. Will retry send up to 5 times."
				RETRY_COUNT=5
				while [ $RETRY_COUNT -gt 0 ]; do
					solana transfer ${pubkey_list[i]} $AMT_TO_TRANSFER --allow-unfunded-recipient
					TEMP_BAL=$(solana balance ${pubkey_list[i]} | awk '{print $1}')
					if [ $(echo "$TEMP_BAL > 0" | bc) -eq 1 ]; then
						break
					else 
						((RETRY_COUNT--))
					fi
				done
			fi
		done
		if [ ${#empty_accts[@]} = 0 ]; then
			SUCCESS_FLAG=true
		fi
		if $SUCCESS_FLAG = true; then
			echo "All accounts are now funded and setup is complete."
			echo "Here is a breakdown of the accounts, associated private keys, and their balances:"; echo ""
			echo "Pubkey                                        Private Key                Balance "
			for ((i=0; i<${#pubkey_list[@]}; i++)) do
				TEMP_BAL=$(solana balance ${pubkey_list[i]} | awk '{print $1}')
				echo "${pubkey_list[i]}  ~/.config/solana/id$((i + 1)).json  $TEMP_BAL SOL"
			done
			break
		else
			echo "Not all accounts could be successfully funded.  Recommend you try manual transactions to fund the remaining empty accounts."
			echo "Here is the current state of all accounts so you can see which ones are still empty."; echo ""
			echo "Pubkey                                        Private Key                Balance "
			for ((i=0; i<${#pubkey_list[@]}; i++)) do
				TEMP_BAL=$(solana balance ${pubkey_list[i]} | awk '{print $1}')
				echo "${pubkey_list[i]}  ~/.config/solana/id$((i + 1)).json  $TEMP_BAL SOL"
			done
			echo "To fund the remaining empty accounts, import the private key located at ~/.config/solana/id1.json into a wallet."
			echo "Then send transactions manually to fund the empty accounts."
			echo "To show the private key for id1.json, use the command cat ~/.config/solana/id1.json"
			exit 0
		fi
	elif [ $choice5 = "exit" ]; then
		exit 0
	else 
		echo "Please choose either to proceed with disbursing the funds or to exit."
	fi
done

exit 0