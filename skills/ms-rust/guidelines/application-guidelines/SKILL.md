---
name: application-guidelines
description: Use when working on application-level error handling with anyhow or eyre, CLI tools and desktop applications, performance optimization using mimalloc allocator, or user-facing features and initialization logic
---

# Application Guidelines



## Applications may use Anyhow or Derivatives (M-APP-ERROR) { #M-APP-ERROR }

<why>To simplify application-level error handling.</why>
<version>0.1</version>

> Note, this guideline is primarily a relaxation and clarification of [M-ERRORS-CANONICAL-STRUCTS].

Applications, and crates in your own repository exclusively used from your application, may use [anyhow](https://github.com/dtolnay/anyhow),
[eyre](https://github.com/eyre-rs/eyre) or similar application-level error crates instead of implementing their own types.

For example, in your application crates you may just re-export and use eyre's common `Result` type, which should be able to automatically
handle all third party library errors, in particular the ones following
[M-ERRORS-CANONICAL-STRUCTS].

```rust,ignore
use eyre::Result;

fn start_application() -> Result<()> {
    start_server()?;
    Ok(())
}
```

Once you selected your application error crate you should switch all application-level errors to that type, and you should not mix multiple
application-level error types.

Libraries (crates used by more than one crate) should always follow [M-ERRORS-CANONICAL-STRUCTS] instead.

[M-ERRORS-CANONICAL-STRUCTS]: ../libs/ux/#M-ERRORS-CANONICAL-STRUCTS



## Use Mimalloc for Apps (M-MIMALLOC-APPS) { #M-MIMALLOC-APPS }

<why>To get significant performance for free.</why>
<version>0.1</version>

Applications should set [mimalloc](https://crates.io/crates/mimalloc) as their global allocator. This usually results in notable performance
increases along allocating hot paths; we have seen up to 25% benchmark improvements.

Changing the allocator only takes a few lines of code. Add mimalloc to your `Cargo.toml` like so:

```toml
[dependencies]
mimalloc = { version = "0.1" } # Or later version if available
```

Then use it from your `main.rs`:

```rust,ignore
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
```



