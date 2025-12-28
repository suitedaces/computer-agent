---
name: safety-guidelines
description: Use when writing unsafe code for novel abstractions, performance, or FFI, ensuring code soundness and preventing undefined behavior, documenting safety requirements and invariants, or reviewing unsafe blocks for correctness with Miri
---

# Safety Guidelines



## Unsafe Implies Undefined Behavior (M-UNSAFE-IMPLIES-UB) { #M-UNSAFE-IMPLIES-UB }

<why>To ensure semantic consistency and prevent warning fatigue.</why>
<version>1.0</version>

The marker `unsafe` may only be applied to functions and traits if misuse implies the risk of undefined behavior (UB).
It must not be used to mark functions that are dangerous to call for other reasons.

```rust
// Valid use of unsafe
unsafe fn print_string(x: *const String) { }

// Invalid use of unsafe
unsafe fn delete_database() { }
```



## Unsafe Needs Reason, Should be Avoided (M-UNSAFE) { #M-UNSAFE }

<why>To prevent undefined behavior, attack surface, and similar 'happy little accidents'.</why>
<version>0.2</version>

You must have a valid reason to use `unsafe`. The only valid reasons are

1) novel abstractions, e.g., a new smart pointer or allocator,
1) performance, e.g., attempting to call `.get_unchecked()`,
1) FFI and platform calls, e.g., calling into C or the kernel, ...

Unsafe code lowers the guardrails used by the compiler, transferring some of the compiler's responsibilities
to the programmer. Correctness of the resulting code relies primarily on catching all mistakes in code review,
which is error-prone. Mistakes in unsafe code may introduce high-severity security vulnerabilities.

You must not use ad-hoc `unsafe` to

- shorten a performant and safe Rust program, e.g., 'simplify' enum casts via `transmute`,
- bypass `Send` and similar bounds, e.g., by doing `unsafe impl Send ...`,
- bypass lifetime requirements via `transmute` and similar.

Ad-hoc here means `unsafe` embedded in otherwise unrelated code. It is of course permissible to create properly designed, sound abstractions doing these things.

In any case, `unsafe` must follow the guidelines outlined below.

### Novel Abstractions

- [ ] Verify there is no established alternative. If there is, prefer that.
- [ ] Your abstraction must be minimal and testable.
- [ ] It must be hardened and tested against ["adversarial code"](https://cheats.rs/#adversarial-code), esp.
  - If they accept closures they must become invalid (e.g., poisoned) if the closure panics
  - They must assume any safe trait is misbehaving, esp. `Deref`, `Clone` and `Drop`.
- [ ] Any use of `unsafe` must be accompanied by plain-text reasoning outlining its safety
- [ ] It must pass [Miri](https://github.com/rust-lang/miri), including adversarial test cases
- [ ] It must follow all other [unsafe code guidelines](https://rust-lang.github.io/unsafe-code-guidelines/)

### Performance

- [ ] Using `unsafe` for performance reasons should only be done after benchmarking
- [ ] Any use of `unsafe` must be accompanied by plain-text reasoning outlining its safety. This applies to both
  calling `unsafe` methods, as well as providing `_unchecked` ones.
- [ ] The code in question must pass [Miri](https://github.com/rust-lang/miri)
- [ ] You must follow the [unsafe code guidelines](https://rust-lang.github.io/unsafe-code-guidelines/)

### FFI

- [ ] We recommend you use an established interop library to avoid `unsafe` constructs
- [ ] You must follow the [unsafe code guidelines](https://rust-lang.github.io/unsafe-code-guidelines/)
- [ ] You must document your generated bindings to make it clear which call patterns are permissible

### Further Reading

- [Nomicon](https://doc.rust-lang.org/nightly/nomicon/)
- [Unsafe Code Guidelines](https://rust-lang.github.io/unsafe-code-guidelines/)
- [Miri](https://github.com/rust-lang/miri)
- ["Adversarial code"](https://cheats.rs/#adversarial-code)



## All Code Must be Sound (M-UNSOUND) { #M-UNSOUND }

<why>To prevent unexpected runtime behavior, leading to potential bugs and incompatibilities.</why>
<version>1.0</version>

Unsound code is seemingly _safe_ code that may produce undefined behavior when called from other safe code, or on its own accord.

> ### <tip></tip> Meaning of 'Safe'
>
> The terms _safe_ and `unsafe` are technical terms in Rust.
>
> A function is _safe_, if its signature does not mark it `unsafe`. That said, _safe_ functions can still be dangerous
> (e.g., `delete_database()`), and `unsafe` ones are, when properly used, usually quite benign (e.g.,`vec.get_unchecked()`).
>
> A function is therefore _unsound_ if it appears _safe_ (i.e., it is not marked `unsafe`), but if _any_ of its calling
> modes would cause undefined behavior. This is to be interpreted in the strictest sense. Even if causing undefined
> behavior is only a 'remote, theoretical possibility' requiring 'weird code', the function is unsound.
>
> Also see [Unsafe, Unsound, Undefined](https://cheats.rs/#unsafe-unsound-undefined).

```rust
// "Safely" converts types
fn unsound_ref<T>(x: &T) -> &u128 {
    unsafe { std::mem::transmute(x) }
}

// "Clever trick" to work around missing `Send` bounds.
struct AlwaysSend<T>(T);
unsafe impl<T> Send for AlwaysSend<T> {}
unsafe impl<T> Sync for AlwaysSend<T> {}
```

Unsound abstractions are never permissible. If you cannot safely encapsulate something, you must expose `unsafe` functions instead, and document proper behavior.

<div class="warning">

No Exceptions

While you may break most guidelines if you have a good enough reason, there are no exceptions in this case: unsound code is never acceptable.

</div>

> ### <tip></tip> It's the Module Boundaries
>
> Note that soundness boundaries equal module boundaries! It is perfectly fine, in an otherwise safe abstraction,
> to have safe functions that rely on behavior guaranteed elsewhere **in the same module**.
>
> ```rust
> struct MyDevice(*const u8);
>
> impl MyDevice {
>     fn new() -> Self {
>        // Properly initializes instance ...
>        # todo!()
>     }
>
>     fn get(&self) -> u8 {
>         // It is perfectly fine to rely on `self.0` being valid, despite this
>         // function in-and-by itself being unable to validate that.
>         unsafe { *self.0 }
>     }
> }
>
> ```



