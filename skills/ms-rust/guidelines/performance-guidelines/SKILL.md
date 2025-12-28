---
name: performance-guidelines
description: Use when identifying and profiling hot paths in your code, optimizing for throughput and CPU cycle efficiency, managing allocation patterns and memory usage, or implementing yield points in long-running async tasks
---

# Performance Guidelines



## Identify, Profile, Optimize the Hot Path Early (M-HOTPATH) { #M-HOTPATH }

<why>To end up with high performance code.</why>
<version>0.1</version>

You should, early in the development process, identify if your crate is performance or COGS relevant. If it is:

- identify hot paths and create benchmarks around them,
- regularly run a profiler collecting CPU and allocation insights,
- document or communicate the most performance sensitive areas.

For benchmarks we recommend [criterion](https://crates.io/crates/criterion) or [divan](https://crates.io/crates/divan).
If possible, benchmarks should not only measure elapsed wall time, but also used CPU time over all threads (this unfortunately
requires manual work and is not supported out of the box by the common benchmark utils).

Profiling Rust on Windows works out of the box with [Intel VTune](https://www.intel.com/content/www/us/en/developer/tools/oneapi/vtune-profiler.html)
and [Superluminal](https://superluminal.eu/). However, to gain meaningful CPU insights you should enable debug symbols for benchmarks in your `Cargo.toml`:

```toml
[profile.bench]
debug = 1
```

Documenting the most performance sensitive areas helps other contributors take better decision. This can be as simple as
sharing screenshots of your latest profiling hot spots.

### Further Reading

- [Performance Tips](https://cheats.rs/#performance-tips)

> ### <tip></tip> How much faster?
>
> Some of the most common 'language related' issues we have seen include:
>
> - frequent re-allocations, esp. cloned, growing or `format!` assembled strings,
> - short lived allocations over bump allocations or similar,
> - memory copy overhead that comes from cloning Strings and collections,
> - repeated re-hashing of equal data structures
> - the use of Rust's default hasher where collision resistance wasn't an issue
>
> Anecdotally, we have seen ~15% benchmark gains on hot paths where only some of these `String`  problems were
> addressed, and it appears that up to 50% could be achieved in highly optimized versions.



## Optimize for Throughput, Avoid Empty Cycles (M-THROUGHPUT) { #M-THROUGHPUT }

<why>To ensure COGS savings at scale.</why>
<version>0.1</version>

You should optimize your library for throughput, and one of your key metrics should be _items per CPU cycle_.

This does not mean to neglect latency&mdash;after all you can scale for throughput, but not for latency. However,
in most cases you should not pay for latency with _empty cycles_ that come with single-item processing, contended locks and frequent task switching.

Ideally, you should

- partition reasonable chunks of work ahead of time,
- let individual threads and tasks deal with their slice of work independently,
- sleep or yield when no work is present,
- design your own APIs for batched operations,
- perform work via batched APIs where available,
- yield within long individual items, or between chunks of batches (see [M-YIELD-POINTS]),
- exploit CPU caches, temporal and spatial locality.

You should not:

- hot spin to receive individual items faster,
- perform work on individual items if batching is possible,
- do work stealing or similar to balance individual items.

Shared state should only be used if the cost of sharing is less than the cost of re-computation.

[M-YIELD-POINTS]: ./#M-YIELD-POINTS



## Long-Running Tasks Should Have Yield Points. (M-YIELD-POINTS) { #M-YIELD-POINTS }

<why>To ensure you don't starve other tasks of CPU time.</why>
<version>0.2</version>

If you perform long running computations, they should contain `yield_now().await` points.

Your future might be executed in a runtime that cannot work around blocking or long-running tasks. Even then, such tasks are
considered bad design and cause runtime overhead. If your complex task performs I/O regularly it will simply utilize these await points to preempt itself:

```rust, ignore
async fn process_items(items: &[items]) {
    // Keep processing items, the runtime will preempt you automatically.
    for i in items {
        read_item(i).await;
    }
}
```

If your task performs long-running CPU operations without intermixed I/O, it should instead cooperatively yield at regular intervals, to not starve concurrent operations:

```rust, ignore
async fn process_items(zip_file: File) {
    let items = zip_file.read().async;
    for i in items {
        decompress(i);
        yield_now().await;
    }
}
```

If the number and duration of your individual operations are unpredictable you should use APIs such as `has_budget_remaining()` and
related APIs to query your hosting runtime.

> ### <tip></tip> Yield how often?
>
> In a thread-per-core model the overhead of task switching must be balanced against the systemic effects of starving unrelated tasks.
>
> Under the assumption that runtime task switching takes 100's of ns, in addition to the overhead of lost CPU caches,
> continuous execution in between should be long enough that the switching cost becomes negligible (<1%).
>
> Thus, performing 10 - 100μs of CPU-bound work between yield points would be a good starting point.



