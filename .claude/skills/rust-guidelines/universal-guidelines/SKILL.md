---
name: universal-guidelines
description: Use in ALL Rust tasks. This file defines general Rust best practices, style, naming, logging, panics, Debug/Display implementations, static verification with clippy, and cross-cutting concerns that apply everywhere
---

# Universal Guidelines



## Names are Free of Weasel Words (M-CONCISE-NAMES) { #M-CONCISE-NAMES }

<why>To improve readability.</why>
<version>1.0</version>

Symbol names, especially types and traits names, should be free of weasel words that do not meaningfully
add information. Common offenders include `Service`, `Manager`, and `Factory`. For example:

While your library may very well contain or communicate with a booking service&mdash;or even hold an `HttpClient`
instance named `booking_service`&mdash;one should rarely encounter a `BookingService` _type_ in code.

An item handling many bookings can just be called `Bookings`. If it does anything more specific, then that quality
should be appended instead. It submits these items elsewhere? Calling it `BookingDispatcher` would be more helpful.

The same is true for `Manager`s. Every code manages _something_, so that moniker is rarely useful. With rare
exceptions, life cycle issues should likewise not be made the subject of some manager. Items are created in whatever
way they are needed, their disposal is governed by `Drop`, and only `Drop`.

Regarding factories, at least the term should be avoided. While the concept `FooFactory` has its use, its canonical
Rust name is `Builder` (compare [M-INIT-BUILDER](../libs/ux/#M-INIT-BUILDER)). A builder that can produce items repeatedly is still a builder.

In addition, accepting factories (builders) as parameters is an unidiomatic import of OO concepts into Rust. If
repeatable instantiation is required, functions should ask for an `impl Fn() -> Foo` over a `FooBuilder` or
similar. In contrast, standalone builders have their use, but primarily to reduce parametric permutation complexity
around optional values (again, [M-INIT-BUILDER](../libs/ux/#M-INIT-BUILDER)).



## Magic Values are Documented (M-DOCUMENTED-MAGIC) { #M-DOCUMENTED-MAGIC }

<why>To ensure maintainability and prevent misunderstandings when refactoring.</why>
<version>1.0</version>

Hardcoded _magic_ values in production code must be accompanied by a comment. The comment should outline:

- why this value was chosen,
- non-obvious side effects if that value is changed,
- external systems that interact with this constant.

You should prefer named constants over inline values.

```rust, ignore
// Bad: it's relatively obvious that this waits for a day, but not why
wait_timeout(60 * 60 * 24).await // Wait at most a day

// Better
wait_timeout(60 * 60 * 24).await // Large enough value to ensure the server
                                 // can finish. Setting this too low might
                                 // make us abort a valid request. Based on
                                 // `api.foo.com` timeout policies.

// Best

/// How long we wait for the server.
///
/// Large enough value to ensure the server
/// can finish. Setting this too low might
/// make us abort a valid request. Based on
/// `api.foo.com` timeout policies.
const UPSTREAM_SERVER_TIMEOUT: Duration = Duration::from_secs(60 * 60 * 24);
```



## Lint Overrides Should Use `#[expect]` (M-LINT-OVERRIDE-EXPECT) { #M-LINT-OVERRIDE-EXPECT }

<why>To prevent the accumulation of outdated lints.</why>
<version>1.0</version>

When overriding project-global lints inside a submodule or item, you should do so via `#[expect]`, not `#[allow]`.

Expected lints emit a warning if the marked warning was not encountered, thus preventing the accumulation of stale lints.
That said, `#[allow]` lints are still useful when applied to generated code, and can appear in macros.

Overrides should be accompanied by a `reason`:

```rust,edition2021
#[expect(clippy::unused_async, reason = "API fixed, will use I/O later")]
pub async fn ping_server() {
  // Stubbed out for now
}
```



## Use Structured Logging with Message Templates (M-LOG-STRUCTURED) { #M-LOG-STRUCTURED }

<why>To minimize the cost of logging and to improve filtering capabilities.</why>
<version>0.1</version>

Logging should use structured events with named properties and message templates following
the [message templates](https://messagetemplates.org/) specification.

> **Note:** Examples use the [`tracing`](https://docs.rs/tracing/) crate's `event!` macro,
but these principles apply to any logging API that supports structured logging (e.g., `log`,
`slog`, custom telemetry systems).

### Avoid String Formatting

String formatting allocates memory at runtime. Message templates defer formatting until viewing time.
We recommend that message template includes all named properties for easier inspection at viewing time.

```rust,ignore
// Bad: String formatting causes allocations
tracing::info!("file opened: {}", path);
tracing::info!(format!("file opened: {}", path));

// Good: Message templates with named properties
event!(
    name: "file.open.success",
    Level::INFO,
    file.path = path.display(),
    "file opened: {{file.path}}",
);
```

> **Note**: Use the `{{property}}` syntax in message templates which preserves the literal text
> while escaping Rust's format syntax. String formatting is deferred until logs are viewed.

### Name Your Events

Use hierarchical dot-notation: `<component>.<operation>.<state>`

```rust,ignore
// Bad: Unnamed events
event!(
    Level::INFO,
    file.path = file_path,
    "file {{file.path}} processed succesfully",
);

// Good: Named events
event!(
    name: "file.processing.success", // event identifier
    Level::INFO,
    file.path = file_path,
    "file {{file.path}} processed succesfully",
);
```

Named events enable grouping and filtering across log entries.

### Follow OpenTelemetry Semantic Conventions

Use [OTel semantic conventions](https://opentelemetry.io/docs/specs/semconv/) for common attributes if needed.
This enables standardization and interoperability.

```rust,ignore
event!(
    name: "file.write.success",
    Level::INFO,
    file.path = path.display(),         // Standard OTel name
    file.size = bytes_written,          // Standard OTel name
    file.directory = dir_path,          // Standard OTel name
    file.extension = extension,         // Standard OTel name
    file.operation = "write",           // Custom name
    "{{file.operation}} {{file.size}} bytes to {{file.path}} in {{file.directory}} extension={{file.extension}}",
);
```

Common conventions:

- HTTP: `http.request.method`, `http.response.status_code`, `url.scheme`, `url.path`, `server.address`
- File: `file.path`, `file.directory`, `file.name`, `file.extension`, `file.size`
- Database: `db.system.name`, `db.namespace`, `db.operation.name`, `db.query.text`
- Errors: `error.type`, `error.message`, `exception.type`, `exception.stacktrace`

### Redact Sensitive Data

Do not log plain sensitive data as this might lead to privacy and security incidents.

```rust,ignore
// Bad: Logs potentially sensitive data
event!(
    name: "file.operation.started",
    Level::INFO,
    user.email = user.email,  // Sensitive data
    file.name = "license.txt",
    "reading file {{file.name}} for user {{user.email}}",
);

// Good: Redact sensitive parts
event!(
    name: "file.operation.started",
    Level::INFO,
    user.email.redacted = redact_email(user.email),
    file.name = "license.txt",
    "reading file {{file.name}} for user {{user.email.redacted}}",
);
```

Sensitive data includes email addresses, file paths revealing user identity, filenames containing secrets or tokens,
file contents with PII, temporary file paths with session IDs and more. Consider using the [`data_privacy`](https://crates.io/crates/data_privacy) crate for consistent redaction.

### Further Reading

- [Message Templates Specification](https://messagetemplates.org/)
- [OpenTelemetry Semantic Conventions](https://opentelemetry.io/docs/specs/semconv/)
- [OWASP Logging Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html)



## Panic Means 'Stop the Program' (M-PANIC-IS-STOP) { #M-PANIC-IS-STOP }

<why>To ensure soundness and predictability.</why>
<version>1.0</version>

Panics are not exceptions. Instead, they suggest immediate program termination.

Although your code must be [panic-safe](https://doc.rust-lang.org/nomicon/exception-safety.html) (i.e., a survived panic may not lead to
inconsistent state), invoking a panic means _this program should stop now_. It is not valid to:

- use panics to communicate (errors) upstream,
- use panics to handle self-inflicted error conditions,
- assume panics will be caught, even by your own code.

For example, if the application calling you is compiled with a `Cargo.toml` containing

```toml
[profile.release]
panic = "abort"
```

then any invocation of panic will cause an otherwise functioning program to needlessly abort. Valid reasons to panic are:

- when encountering a programming error, e.g., `x.expect("must never happen")`,
- anything invoked from const contexts, e.g., `const { foo.unwrap() }`,
- when user requested, e.g., providing an `unwrap()` method yourself,
- when encountering a poison, e.g., by calling `unwrap()` on a lock result (a poisoned lock signals another thread has panicked already).

Any of those are directly or indirectly linked to programming errors.



## Detected Programming Bugs are Panics, Not Errors (M-PANIC-ON-BUG) { #M-PANIC-ON-BUG }

<why>To avoid impossible error handling code and ensure runtime consistency.</why>
<version>1.0</version>

As an extension of [M-PANIC-IS-STOP] above, when an unrecoverable programming error has been
detected, libraries and applications must panic, i.e., request program termination.

In these cases, no `Error` type should be introduced or returned, as any such error could not be acted upon at runtime.

Contract violations, i.e., the breaking of invariants either within a library or by a caller, are programming errors and must therefore panic.

However, what constitutes a violation is situational. APIs are not expected to go out of their way to detect them, as such
checks can be impossible or expensive. Encountering `must_be_even == 3` during an already existing check clearly warrants
a panic, while a function `parse(&str)` clearly must return a `Result`. If in doubt, we recommend you take inspiration from the standard library.

```rust, ignore
// Generally, a function with bad parameters must either
// - Ignore a parameter and/or return the wrong result
// - Signal an issue via Result or similar
// - Panic
// If in this `divide_by` we see that y == 0, panicking is
// the correct approach.
fn divide_by(x: u32, y: u32) -> u32 { ... }

// However, it can also be permissible to omit such checks
// and return an unspecified (but not an undefined) result.
fn divide_by_fast(x: u32, y: u32) -> u32 { ... }

// Here, passing an invalid URI is not a contract violation.
// Since parsing is inherently fallible, a Result must be returned.
fn parse_uri(s: &str) -> Result<Uri, ParseError> { };

```

> ### <tip></tip> Make it 'Correct by Construction'
>
> While panicking on a detected programming error is the 'least bad option', your panic might still ruin someone's day.
> For any user input or calling sequence that would otherwise panic, you should also explore if you can use the type
> system to avoid panicking code paths altogether.

[M-PANIC-IS-STOP]: ../universal/#M-PANIC-IS-STOP



## Public Types are Debug (M-PUBLIC-DEBUG) { #M-PUBLIC-DEBUG }

<why>To simplify debugging and prevent leaking sensitive data.</why>
<version>1.0</version>

All public types exposed by a crate should implement `Debug`. Most types can do so via `#[derive(Debug)]`:

```rust
#[derive(Debug)]
struct Endpoint(String);
```

Types designed to hold sensitive data should also implement `Debug`, but do so via a custom implementation.
This implementation must employ unit tests to ensure sensitive data isn't actually leaked, and will not be in the future.

```rust
use std::fmt::{Debug, Formatter};

struct UserSecret(String);

impl Debug for UserSecret {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "UserSecret(...)")
    }
}

#[test]
fn test() {
    let key = "552d3454-d0d5-445d-ab9f-ef2ae3a8896a";
    let secret = UserSecret(key.to_string());
    let rendered = format!("{:?}", secret);

    assert!(rendered.contains("UserSecret"));
    assert!(!rendered.contains(key));
}
```



## Public Types Meant to be Read are Display (M-PUBLIC-DISPLAY) { #M-PUBLIC-DISPLAY }

<why>To improve usability.</why>
<version>1.0</version>

If your type is expected to be read by upstream consumers, be it developers or end users, it should implement `Display`. This in particular includes:

- Error types, which are mandated by `std::error::Error` to implement `Display`
- Wrappers around string-like data

Implementations of `Display` should follow Rust customs; this includes rendering newlines and escape sequences.
The handling of sensitive data outlined in [M-PUBLIC-DEBUG] applies analogously.

[M-PUBLIC-DEBUG]: ./#M-PUBLIC-DEBUG



## Prefer Regular over Associated Functions (M-REGULAR-FN) { #M-REGULAR-FN }

<why>To improve readability.</why>
<version>1.0</version>

Associated functions should primarily be used for instance creation, not general purpose computation.

In contrast to some OO languages, regular functions are first-class citizens in Rust and need no module or _class_ to host them. Functionality that
does not clearly belong to a receiver should therefore not reside in a type's `impl` block:

```rust, ignore
struct Database {}

impl Database {
    // Ok, associated function creates an instance
    fn new() -> Self {}

    // Ok, regular method with `&self` as receiver
    fn query(&self) {}

    // Not ok, this function is not directly related to `Database`,
    // it should therefore not live under `Database` as an associated
    // function.
    fn check_parameters(p: &str) {}
}

// As a regular function this is fine
fn check_parameters(p: &str) {}
```

Regular functions are more idiomatic, and reduce unnecessary noise on the caller side. Associated trait functions are perfectly idiomatic though:

```rust
pub trait Default {
    fn default() -> Self;
}

struct Foo;

impl Default for Foo {
    fn default() -> Self { Self }
}
```



## If in Doubt, Split the Crate (M-SMALLER-CRATES) { #M-SMALLER-CRATES }

<why>To improve compile times and modularity.</why>
<version>1.0</version>

You should err on the side of having too many crates rather than too few, as this leads to dramatic compile time improvements—especially
during the development of these crates—and prevents cyclic component dependencies.

Essentially, if a submodule can be used independently, its contents should be moved into a separate crate.

Performing this crate split may cause you to lose access to some `pub(crate)` fields or methods. In many situations, this is a desirable
side-effect and should prompt you to design more flexible abstractions that would give your users similar affordances.

In some cases, it is desirable to re-join individual crates back into a single _umbrella crate_, such as when dealing with proc macros, or runtimes.
Functionality split for technical reasons (e.g., a `foo_proc` proc macro crate) should always be re-exported. Otherwise, re-exports should be used sparingly.

> ### <tip></tip> Features vs. Crates
>
> As a rule of thumb, crates are for items that can reasonably be used on their own. Features should unlock extra functionality that
> can't live on its own. In the case of umbrella crates, see below, features may also be used to enable constituents (but then that functionality
> was extracted into crates already).
>
> For example, if you defined a `web` crate with the following modules, users only needing client calls would also have to pay for the compilation of server code:
>
> ```text
> web::server
> web::client
> web::protocols
> ```
>
> Instead, you should introduce individual crates that give users the ability to pick and choose:
>
> ```text
> web_server
> web_client
> web_protocols
> ```



## Use Static Verification (M-STATIC-VERIFICATION) { #M-STATIC-VERIFICATION }

<why>To ensure consistency and avoid common issues.</why>
<version>1.0</version>

Projects should use the following static verification tools to help maintain the quality of the code. These tools can be
configured to run on a developer's machine during normal work, and should be used as part of check-in gates.

* [compiler lints](https://doc.rust-lang.org/rustc/lints/index.html) offer many lints to avoid bugs and improve code quality.
* [clippy lints](https://doc.rust-lang.org/clippy/) contain hundreds of lints to avoid bugs and improve code quality.
* [rustfmt](https://github.com/rust-lang/rustfmt) ensures consistent source formatting.
* [cargo-audit](https://crates.io/crates/cargo-audit) verifies crate dependencies for security vulnerabilities.
* [cargo-hack](https://crates.io/crates/cargo-hack) validates that all combinations of crate features work correctly.
* [cargo-udeps](https://crates.io/crates/cargo-udeps) detects unused dependencies in Cargo.toml files.
* [miri](https://github.com/rust-lang/miri) validates the correctness of unsafe code.

### Compiler Lints

The Rust compiler generally produces exceptionally good diagnostics. In addition to the default set of diagnostics, projects
should explicitly enable the following set of compiler lints:

```toml
[lints.rust]
ambiguous_negative_literals = "warn"
missing_debug_implementations = "warn"
redundant_imports = "warn"
redundant_lifetimes = "warn"
trivial_numeric_casts = "warn"
unsafe_op_in_unsafe_fn = "warn"
unused_lifetimes = "warn"
```

### Clippy Lints

For clippy, projects should enable all major lint categories, and additionally enable some lints from the `restriction` lint group.
Undesired lints (e.g., numeric casts) can be opted back out of on a case-by-case basis:

```toml
[lints.clippy]
cargo = { level = "warn", priority = -1 }
complexity = { level = "warn", priority = -1 }
correctness = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
perf = { level = "warn", priority = -1 }
style = { level = "warn", priority = -1 }
suspicious = { level = "warn", priority = -1 }
# nursery = { level = "warn", priority = -1 }  # optional, might cause more false positives

# These lints are from the `restriction` lint group and prevent specific
# constructs being used in source code in order to drive up consistency,
# quality, and brevity
allow_attributes_without_reason = "warn"
as_pointer_underscore = "warn"
assertions_on_result_states = "warn"
clone_on_ref_ptr = "warn"
deref_by_slicing = "warn"
disallowed_script_idents = "warn"
empty_drop = "warn"
empty_enum_variants_with_brackets = "warn"
empty_structs_with_brackets = "warn"
fn_to_numeric_cast_any = "warn"
if_then_some_else_none = "warn"
map_err_ignore = "warn"
redundant_type_annotations = "warn"
renamed_function_params = "warn"
semicolon_outside_block = "warn"
string_to_string = "warn"
undocumented_unsafe_blocks = "warn"
unnecessary_safety_comment = "warn"
unnecessary_safety_doc = "warn"
unneeded_field_pattern = "warn"
unused_result_ok = "warn"

# May cause issues with structured logging otherwise.
literal_string_with_formatting_args = "allow"

# Define custom opt outs here
# ...
```



## Follow the Upstream Guidelines (M-UPSTREAM-GUIDELINES) { #M-UPSTREAM-GUIDELINES }

<why>To avoid repeating mistakes the community has already learned from, and to have a codebase that does not surprise users and contributors.</why>
<version>1.0</version>

The guidelines in this book complement existing Rust guidelines, in particular:

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html)
- [Rust Style Guide](https://doc.rust-lang.org/nightly/style-guide/)
- [Rust Design Patterns](https://rust-unofficial.github.io/patterns//intro.html)
- [Rust Reference - Undefined Behavior](https://doc.rust-lang.org/reference/behavior-considered-undefined.html)

We recommend you read through these as well, and apply them in addition to this book's items. Pay special attention to the ones below, as they are frequently forgotten:

- [ ] [C-CONV](https://rust-lang.github.io/api-guidelines/naming.html#ad-hoc-conversions-follow-as_-to_-into_-conventions-c-conv) - Ad-hoc conversions
  follow  `as_`, `to_`, `into_` conventions
- [ ] [C-GETTER](https://rust-lang.github.io/api-guidelines/naming.html#getter-names-follow-rust-convention-c-getter) - Getter names follow Rust convention
- [ ] [C-COMMON-TRAITS](https://rust-lang.github.io/api-guidelines/interoperability.html#c-common-traits) - Types eagerly implement common traits
  - `Copy`, `Clone`, `Eq`, `PartialEq`, `Ord`, `PartialOrd`, `Hash`, `Default`, `Debug`
  - `Display` where type wants to be displayed
- [ ] [C-CTOR](https://rust-lang.github.io/api-guidelines/predictability.html?highlight=new#constructors-are-static-inherent-methods-c-ctor) -
  Constructors are static, inherent methods
  - In particular, have `Foo::new()`, even if you have `Foo::default()`
- [ ] [C-FEATURE](https://rust-lang.github.io/api-guidelines/naming.html#feature-names-are-free-of-placeholder-words-c-feature) - Feature names
  are free of placeholder words



