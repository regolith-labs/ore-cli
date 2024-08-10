
# Key Changes

## Faster submissions

## Thread Affinity and Load Balancing:

Switched from pinning threads to specific CPU cores to a thread pool model, ensuring better utilization of all available CPU cores.

Introduced a channel-based mechanism to collect results from worker threads, allowing for dynamic workload distribution across cores.

Improved overall efficiency and balanced the workload, reducing the likelihood of underutilized cores.

## Optimized Hashing Algorithm:

Reduced memory allocations and minimized memory access within the hashing loop by reusing buffers and optimizing loop operations.

Increased the nonce increment step to process batches of nonces in a single loop iteration, reducing loop overhead.

## Compiler-Level CPU Optimizations:

Modified the Cargo.toml file to include compiler flags that optimize the code for the specific CPU architecture in use (target-cpu=native).

Enabled Link Time Optimization (LTO) and other advanced optimizations (like reducing codegen units and setting opt-level to maximize performance).

These changes ensure that the compiled binary takes full advantage of all available CPU features, such as AVX2 and SSE4.2, leading to potentially higher hash rates and improved overall performance.




check out the repo and build
```sh
cargo build --release
```

## Help

You can use the `-h` flag on any command to pull up a help menu with documentation:

```sh
ore -h
```
