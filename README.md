
# Key Changes

Replace std::thread::spawn with tokio::spawn: This will make tasks asynchronous and non-blocking, allowing for more efficient task management.

Use tokio::sync::RwLock instead of std::sync::RwLock: This change is necessary for asynchronous contexts.

Implement async version of find_hash_par using Tokio: This will enable spawning tasks across cores without blocking the main event loop.


```sh
cargo build --release
```

## Help

You can use the `-h` flag on any command to pull up a help menu with documentation:

```sh
ore -h
```
