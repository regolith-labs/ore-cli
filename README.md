# Ore CLI
A command line interface for the Ore program.

## Building the utility
To build the Ore CLI, you will need to have the Rust programming language installed. You can install Rust by following the instructions on the [Rust website](https://www.rust-lang.org/tools/install).

Once you have Rust installed, you can build the Ore CLI by running the following command in the ore-cli folder:
```sh
./build_and_mine.sh nomine
```
The first build can be slow so please be patient while each library is compiled. Subsequent rebuilds will be significantly quicker.
If the compilation fails, errors will be shown on screen for you to rectify.

The build process creates a commpiled ore cli in the path ```./target/release/ore```. This is the ore cli utility

## Rebuilding the utility
Save your edits to the source code then execute ```./build_and_mine.sh```. If the build is successful, a mining session will automatically be started.

## Manually starting a mining session
Executing the command:
```sh
./miner.sh
```
