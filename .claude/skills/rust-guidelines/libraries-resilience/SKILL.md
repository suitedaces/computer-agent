---
name: libraries-resilience
description: Use when avoiding statics and thread-local state in libraries, making I/O and system calls mockable for testing, preventing glob re-exports and accidental leaks, or feature-gating test utilities and mocking functionality
---

# Libraries / Resilience Guidelines



## Avoid Statics (M-AVOID-STATICS) { #M-AVOID-STATICS }

<why>To prevent consistency and correctness issues between crate versions.</why>
<version>1.0</version>

Libraries should avoid `static` and thread-local items, if a consistent view of the item is relevant for correctness.
Essentially, any code that would be incorrect if the static _magically_ had another value must not use them. Statics
only used for performance optimizations are ok.

The fundamental issue with statics in Rust is the secret duplication of state.

Consider a crate `core` with the following function:

```rust
# use std::sync::atomic::AtomicUsize;
# use std::sync::atomic::Ordering;
static GLOBAL_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn increase_counter() -> usize {
    GLOBAL_COUNTER.fetch_add(1, Ordering::Relaxed)
}
```

Now assume you have a crate `main`, calling two libraries `library_a` and `library_b`, each invoking that counter:

```rust,ignore
// Increase global static counter 2 times
library_a::count_up();
library_a::count_up();

// Increase global static counter 3 more times
library_b::count_up();
library_b::count_up();
library_b::count_up();
```

They eventually report their result:

```rust,ignore
library_a::print_counter();
library_b::print_counter();
main::print_counter();
```

At this point, what is _the_ value of said counter; `0`, `2`, `3` or `5`?

The answer is, possibly any  (even multiple!) of the above, depending on the crate's version resolution!

Under the hood Rust may link to multiple versions of the same crate, independently instantiated, to satisfy declared
dependencies. This is especially observable during a crate's `0.x` version timeline, where each `x` constitutes a separate _major_ version.

If `main`,  `library_a` and `library_b` all declared the same version of `core`, e.g. `0.5`, then the reported result will be `5`, since all
crates actually _see_ the same version of `GLOBAL_COUNTER`.

However, if `library_a` declared `0.4` instead, then it would be linked against a separate version of `core`; thus `main` and `library_b` would
agree on a value of `3`, while `library_a` reported `2`.

Although `static` items can be useful, they are particularly dangerous before a library's stabilization, and for any state where _secret duplication_ would
cause consistency issues when static and non-static variable use interacts. In addition, statics interfere with unit testing, and are a contention point in
thread-per-core designs.



## I/O and System Calls Are Mockable (M-MOCKABLE-SYSCALLS) { #M-MOCKABLE-SYSCALLS }

<why>To make otherwise hard-to-evoke edge cases testable.</why>
<version>0.2</version>

Any user-facing type doing I/O, or sys calls with side effects, should be mockable to these effects. This includes file and
network access, clocks, entropy sources and seeds, and similar. More generally, any operation that is

- non-deterministic,
- reliant on external state,
- depending on the hardware or the environment,
- is otherwise fragile or not universally reproducible

should be mockable.

> ### <tip></tip> Mocking Allocations?
>
> Unless you write kernel code or similar, you can consider allocations to be deterministic, hardware independent and practically
> infallible, thus not covered by this guideline.
>
> However, this does _not_ mean you should expect there to be unlimited memory available. While it is ok to
> accept caller provided input as-is if your library has a _reasonable_ memory complexity, memory-hungry libraries
> and code handling external input should provide bounded and / or chunking operations.

This guideline has several implications for libraries, they

- should not perform ad-hoc I/O, i.e., call `read("foo.txt")`
- should not rely on non-mockable I/O and sys calls
- should not create their own I/O or sys call _core_ themselves
- should not offer `MyIoLibrary::default()` constructors

Instead, libraries performing I/O and sys calls should either accept some I/O _core_ that is mockable already, or provide mocking functionality themselves:

```rust, ignore
let lib = Library::new_runtime(runtime_io); // mockable I/O functionality passed in
let (lib, mock) = Library::new_mocked(); // supports inherent mocking
```

Libraries supporting inherent mocking should implement it as follows:

```rust, ignore
pub struct Library {
    some_core: LibraryCore // Encapsulates syscalls, I/O, ... compare below.
}

impl Library {
    pub fn new() -> Self { ... }
    pub fn new_mocked() -> (Self, MockCtrl) { ... }
}
```

Behind the scenes, `LibraryCore` is a non-public enum, similar to [M-RUNTIME-ABSTRACTED], that either dispatches
calls to the respective sys call, or to an mocking controller.

```rust, ignore
// Dispatches calls either to the operating system, or to a
// mocking controller.
enum LibraryCore {
    Native,

    #[cfg(feature = "test-util")]
    Mocked(mock::MockCtrl)
}

impl LibraryCore {
    // Some function you'd forward to the operating system.
    fn random_u32(&self) {
        match self {
            Self::Native => unsafe { os_random_u32() }
            Self::Mocked(m) => m.random_u32()
        }
    }
}


#[cfg(feature = "test-util")]
mod mock {
    // This follows the M-SERVICES-CLONE pattern, so both `LibraryCore` and
    // the user can hold on to the same `MockCtrl` instance.
    pub struct MockCtrl {
        inner: Arc<MockCtrlInner>
    }

    // Implement required logic accordingly, usually forwarding to
    // `MockCtrlInner` below.
    impl MockCtrl {
        pub fn set_next_u32(&self, x: u32) { ... }
        pub fn random_u32(&self) { ... }
    }

    // Contains actual logic, e.g., the next random number we should return.
    struct MockCtrlInner {
        next_call: u32
    }
}
```

Runtime-aware libraries already build on top of the [M-RUNTIME-ABSTRACTED] pattern should extend their runtime enum instead:

```rust, ignore
enum Runtime {
    #[cfg(feature="tokio")]
    Tokio(tokio::Tokio),

    #[cfg(feature="smol")]
    Smol(smol::Smol)

    #[cfg(feature="test-util")]
    Mock(mock::MockCtrl)
}
```

As indicated above, most libraries supporting mocking should not accept mock controllers, but return them via parameter tuples,
with the first parameter being the library instance, the second the mock controller. This is to prevent state ambiguity if multiple
instances shared a single controller:

```rust, ignore
impl Library {
    pub fn new_mocked() -> (Self, MockCtrl) { ... } // good
    pub fn new_mocked_bad(&mut MockCtrl) -> Self { ... } // prone to misuse
}
```

[M-RUNTIME-ABSTRACTED]: ../ux/#M-RUNTIME-ABSTRACTED



## Don't Glob Re-Export Items (M-NO-GLOB-REEXPORTS) { #M-NO-GLOB-REEXPORTS }

<why>To prevent accidentally leaking unintended types.</why>
<version>1.0</version>

Don't `pub use foo::*` from other modules, especially not from other crates. You might accidentally export more than you want,
and globs are hard to review in PRs. Re-export items individually instead:

```rust,ignore
pub use foo::{A, B, C};
```

Glob exports are permissible for technical reasons, like doing platform specific re-exports from a set of HAL (hardware abstraction layer) modules:

```rust,ignore
#[cfg(target_os = "windows")]
mod windows { /* ... */ }

#[cfg(target_os = "linux")]
mod linux { /* ... */ }

// Acceptable use of glob re-exports, this is a common pattern
// and it is clear everything is just forwarded from a single 
// platform.

#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "linux")]
pub use linux::*;
```



## Use the Proper Type Family (M-STRONG-TYPES) { #M-STRONG-TYPES }

<why>To have and maintain the right data and safety variants, at the right time.</why>
<version>1.0</version>

Use the appropriate `std` type for your task. In general you should use the strongest type available, as early as possible in your API flow. Common offenders are

| Do not use ... | use instead ... | Explanation |
| --- | --- | --- |
| `String`* | `PathBuf`* | Anything dealing with the OS should be `Path`-like |

That said, you should also follow common Rust `std` conventions. Purely numeric types at public API boundaries (e.g., `window_size()`) are expected to
be regular numbers, not `Saturating<usize>`, `NonZero<usize>`, or similar.

<footnotes>

<sup>*</sup> Including their siblings, e.g., `&str`, `Path`, ...

</footnotes>



## Test Utilities are Feature Gated (M-TEST-UTIL) { #M-TEST-UTIL }

<why>To prevent production builds from accidentally bypassing safety checks.</why>
<version>0.2</version>

Testing functionality must be guarded behind a feature flag. This includes

- mocking functionality ([M-MOCKABLE-SYSCALLS]),
- the ability to inspect sensitive data,
- safety check overrides,
- fake data generation.

We recommend you use a single flag only, named `test-util`. In any case, the feature(s) must clearly communicate they are for testing purposes.

```rust, ignore
impl HttpClient {
    pub fn get() { ... }

    #[cfg(feature = "test-util")]
    pub fn bypass_certificate_checks() { ... }
}
```

[M-MOCKABLE-SYSCALLS]: ./#M-MOCKABLE-SYSCALLS



