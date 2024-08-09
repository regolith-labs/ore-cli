

## Key Changes:

1. Initial Optimization:
Thread Management and Efficiency: We focused on improving thread management by reducing contention and ensuring work was evenly distributed among threads.
Lock Optimization: We minimized the use of locks or switched to more efficient locking mechanisms to avoid performance bottlenecks.
Load Balancing: Ensured that the mining workload was evenly distributed across the available cores.

3. Integration with Tokio:
Asynchronous Runtime: We integrated Tokio to handle asynchronous tasks and improve overall efficiency. This included using tokio::task::spawn_blocking for CPU-bound tasks.
Tokio's RwLock: We replaced the standard std::sync::RwLock with tokio::sync::RwLock to ensure compatibility with the async runtime.
Improved Task Management: We utilized Tokio's task management to more effectively handle concurrency and task scheduling.

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
