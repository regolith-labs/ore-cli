

## Key Changes:
Thread Efficiency: The threads now have a better distribution of the workload, using workload_per_core to distribute nonces more evenly.

Lock Optimization: The lock contention on global_best_difficulty is reduced by checking and updating the difficulty only when a new best difficulty is found.

Streamlined Logic: Removed unnecessary conditions and streamlined the loop to reduce overhead.

Improved Readability: Cleaned up the code structure, making it easier to read and maintain.

This should result in better performance, especially in multi-core environments, by reducing contention and ensuring that each thread works efficiently without stepping on each other’s toes.

## Build

To build the codebase from scratch, checkout the repo and use cargo to build:


```sh
cargo build --release
```

## Help

You can use the `-h` flag on any command to pull up a help menu with documentation:

```sh
ore -h
```
