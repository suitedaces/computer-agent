---
name: libraries-building
description: Use when creating reusable library crates, managing Cargo features and their additivity, building native sys crates for C interoperability, or ensuring libraries work out-of-the-box on all platforms
---

# Libraries / Building Guidelines



## Features are Additive  (M-FEATURES-ADDITIVE) { #M-FEATURES-ADDITIVE }

<why>To prevent compilation breakage in large and complex projects.</why>
<version>1.0</version>

All library features must be additive, and any combination must work, as long as the feature itself would work on the current platform. This implies:

- [ ] You must not introduce a `no-std` feature, use a `std` feature instead
- [ ] Adding any feature `foo` must not disable or modify any public item
  - Adding enum variants is fine if these enums are `#[non_exhaustive]`
- [ ] Features must not rely on other features to be manually enabled
- [ ] Features must not rely on their parent to skip-enable a feature in one of their children

Further Reading

- [Feature Unification](https://doc.rust-lang.org/cargo/reference/features.html#feature-unification)
- [Mutually Exclusive Features](https://doc.rust-lang.org/cargo/reference/features.html#mutually-exclusive-features)



## Libraries Work Out of the Box (M-OOBE) { #M-OOBE }

<why>To be easily adoptable by the Rust ecosystem.</why>
<version>1.0</version>

Libraries must _just work_ on all supported platforms, with the exception of libraries that are expressly platform or target specific.

Rust crates often come with dozens of dependencies, applications with 100's. Users expect `cargo build` and `cargo install`
to _just work_. Consider this installation of `bat` that pulls in ~250 dependencies:

```text
Compiling writeable v0.5.5
Compiling strsim v0.11.1
Compiling litemap v0.7.5
Compiling crossbeam-utils v0.8.21
Compiling icu_properties_data v1.5.1
Compiling ident_case v1.0.1
Compiling once_cell v1.21.3
Compiling icu_normalizer_data v1.5.1
Compiling fnv v1.0.7
Compiling regex-syntax v0.8.5
Compiling anstyle v1.0.10
Compiling vcpkg v0.2.15
Compiling utf8parse v0.2.2
Compiling aho-corasick v1.1.3
Compiling utf16_iter v1.0.5
Compiling hashbrown v0.15.2
Building [==>                       ] 29/251: icu_locid_transform_data, serde, winnow, indexma...
```

This compilation, like practically all other applications and libraries, will _just work_.

While there are tools targeting specific functionality (e.g., a Wayland compositor) or platform crates like
`windows`; unless a crate is _obviously_ platform specific, the expectation is that it will otherwise _just work_.

This means crates must build, ultimately

- [ ] on all [Tier 1 platforms](https://doc.rust-lang.org/rustc/platform-support.html),<sup>1</sup> and
- [ ] without any additional prerequisites beyond `cargo` and `rust`.<sup>2</sup>

<footnotes>

<sup>1</sup> It is ok to not support Tier 1 platforms "for now", but abstractions must be present so support can easily be extended. This is usually
done by introducing an internal `HAL` ([Hardware Abstraction Layer](https://en.wikipedia.org/wiki/HAL_(software))) module with a `dummy` fallback target.<br/>
<sup>2</sup> A default Rust installation will also have `cc` and a linker present.

</footnotes>

In particular, non-platform crates must not, by default, require the user to install additional tools, or expect environment variables
to compile. If tools were somehow needed (like the generation of Rust from `.proto` files) these tools should be run as part of the
publishing workflow or earlier, and the resulting artifacts (e.g., `.rs` files) be contained inside the published crate.

If a dependency is known to be platform specific, the parent must use conditional (platform) compilation or opt-in feature gates.

> **<alert></alert> Libraries are Responsible for Their Dependencies.**
>
> Imagine you author a `Copilot` crate, which in turn uses an `HttpClient`, which in turn depends on a `perl` script to compile.
>
> Then every one of your users, and your user's users, and everyone above, would need to install Perl to compile _their_ crate. In large projects you would
> have 100's of people who don't know or don't care about your library or Perl, encounter a cryptic compilation error, and now have to figure out how to
> install it on their system.
>
> In practical terms, such behavior is largely a self-inflicted death sentence in the open source space, since the moment alternatives
> are available, people will switch to those that _just work_.



## Native `-sys` Crates Compile Without Dependencies (M-SYS-CRATES) { #M-SYS-CRATES }

<why>To have libraries that 'just work' on all platforms.</why>
<version>0.2</version>

If you author a pair of `foo` and `foo-sys` crates wrapping a native `foo.lib`, you are likely to run into the issues described
in [M-OOBE].

Follow these steps to produce a crate that _just works_ across platforms:

- [ ] fully govern the build of `foo.lib` from `build.rs` inside `foo-sys`. Only use hand-crafted compilation via the
  [cc](https://crates.io/crates/cc) crate, do _not_ run Makefiles or external build scripts, as that will require the installation of external dependencies,
- [ ] make all external tools optional, such as `nasm`,
- [ ] embed the upstream source code in your crate,
- [ ] make the embedded sources verifiable (e.g., include Git URL + hash),
- [ ] pre-generate `bindgen` glue if possible,
- [ ] support both static linking, and dynamic linking via [libloading](https://crates.io/crates/libloading).

Deviations from these points can work, and can be considered on a case-by-case basis:

If the native build system is available as an _OOBE_ crate, that can be used instead of `cc` invocations. The same applies to external tools.

Source code might have to be downloaded if it does not fit crates.io size limitations. In any case, only servers with an availability
comparable to crates.io should be used. In addition, the specific hashes of acceptable downloads should be stored in the crate and verified.

Downloading sources can fail on hermetic build environments, therefore alternative source roots should also be specifiable (e.g., via environment variables).

[M-OOBE]: ./#M-OOBE



