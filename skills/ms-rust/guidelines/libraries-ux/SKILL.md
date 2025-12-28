---
name: libraries-ux
description: Use when designing user-friendly library APIs, managing error types and error handling patterns, creating runtime abstractions and trait-based designs, implementing builders for complex initialization, or structuring crate organization
---

# Libraries / UX Guidelines



## Avoid Smart Pointers and Wrappers in APIs (M-AVOID-WRAPPERS) { #M-AVOID-WRAPPERS }

<why>To reduce cognitive load and improve API ergonomics.</why>
<version>1.0</version>

As a specialization of [M-ABSTRACTIONS-DONT-NEST], generic wrappers and smart pointers like
`Rc<T>`, `Arc<T>`, `Box<T>`, or `RefCell<T>` should be avoided in public APIs.

From a user perspective these are mostly implementation details, and introduce infectious complexity that users have to
resolve. In fact, these might even be impossible to resolve once multiple crates disagree about the required type of wrapper.

If wrappers are needed internally, they should be hidden behind a clean API that uses simple types like `&T`, `&mut T`, or `T` directly. Compare:

```rust,ignore
// Good: simple API
pub fn process_data(data: &Data) -> State { ... }
pub fn store_config(config: Config) -> Result<(), Error> { ... }

// Bad: Exposing implementation details
pub fn process_shared(data: Arc<Mutex<Shared>>) -> Box<Processed> { ... }
pub fn initialize(config: Rc<RefCell<Config>>) -> Arc<Server> { ... }
```

Smart pointers in APIs are acceptable when:

- The smart pointer is fundamental to the API's purpose (e.g., a new container lib)

- The smart pointer, based on benchmarks, significantly improves performance and the complexity is justified.

[M-ABSTRACTIONS-DONT-NEST]: ./#M-ABSTRACTIONS-DONT-NEST



## Prefer Types over Generics, Generics over Dyn Traits (M-DI-HIERARCHY) { #M-DI-HIERARCHY }

<why>To prevent patterns that don't compose, and design lock-in.</why>
<version>0.1</version>

When asking for async dependencies, prefer concrete types over generics, and generics over `dyn Trait`.

It is easy to accidentally deviate from this pattern when porting code from languages like C# that heavily rely on interfaces.
Consider you are porting a service called `Database` from C# to Rust and, inspired by the original `IDatabase` interface, you naively translate it into:

```rust,ignore
trait Database {
    async fn update_config(&self, file: PathBuf);
    async fn store_object(&self, id: Id, obj: Object);
    async fn load_object(&self, id: Id) -> Object;
}

impl Database for MyDatabase { ... }

// Intended to be used like this:
async fn start_service(b: Rc<dyn Database>) { ... }
```

Apart from not feeling idiomatic, this approach precludes other Rust constructs that conflict with object safety,
can cause issues with asynchronous code, and exposes wrappers (compare [M-AVOID-WRAPPERS]).

Instead, when more than one implementation is needed, this _design escalation ladder_ should be followed:

If the other implementation is only concerned with providing a _sans-io_ implementation for testing, implement your type as an
enum, following [M-MOCKABLE-SYSCALLS] instead.

If users are expected to provide custom implementations, you should introduce one or more traits, and implement them for your own types
_on top_ of your inherent functions. Each trait should be relatively narrow, e.g., `StoreObject`, `LoadObject`. If eventually a single
trait is needed it should be made a subtrait, e.g., `trait DataAccess: StoreObject + LoadObject {}`.

Code working with these traits should ideally accept them as generic type parameters as long as their use does not contribute to significant nesting
(compare [M-ABSTRACTIONS-DONT-NEST]).

```rust,ignore
// Good, generic does not have infectious impact, uses only most specific trait
async fn read_database(x: impl LoadObject) { ... }

// Acceptable, unless further nesting makes this excessive.
struct MyService<T: DataAccess> {
    db: T,
}
```

Once generics become a nesting problem, `dyn Trait` can be considered. Even in this case, visible wrapping should be avoided, and custom wrappers should be preferred.

```rust
# use std::sync::Arc;
# trait DataAccess {
#     fn foo(&self);
# }
// This allows you to expand or change `DynamicDataAccess` later. You can also
// implement `DataAccess` for `DynamicDataAccess` if needed, and use it with
// regular generic functions.
struct DynamicDataAccess(Arc<dyn DataAccess>);

impl DynamicDataAccess {
    fn new<T: DataAccess + 'static>(db: T) -> Self {
        Self(Arc::new(db))
    }
}

struct MyService {
    db: DynamicDataAccess,
}
```

The generic wrapper can also be combined with the enum approach from [M-MOCKABLE-SYSCALLS]:

```rust,ignore
enum DataAccess {
    MyDatabase(MyDatabase),
    Mock(mock::MockCtrl),
    Dynamic(DynamicDataAccess)
}

async fn read_database(x: &DataAccess) { ... }
```

[M-AVOID-WRAPPERS]: ./#M-AVOID-WRAPPERS
[M-MOCKABLE-SYSCALLS]: ../resilience/#M-MOCKABLE-SYSCALLS
[M-ABSTRACTIONS-DONT-NEST]: ./#M-ABSTRACTIONS-DONT-NEST



## Error are Canonical Structs (M-ERRORS-CANONICAL-STRUCTS) { #M-ERRORS-CANONICAL-STRUCTS }

<why>To harmonize the behavior of error types, and provide a consistent error handling.</why>
<version>1.0</version>

Errors should be a situation-specific `struct` that contain a [`Backtrace`](https://doc.rust-lang.org/stable/std/backtrace/struct.Backtrace.html),
a possible upstream error cause, and helper methods.

Simple crates usually expose a single error type `Error`, complex crates may expose multiple types, for example
`AccessError` and `ConfigurationError`. Error types should provide helper methods for additional information that allows callers to handle the error.

A simple error might look like so:

```rust
# use std::backtrace::Backtrace;
# use std::fmt::Display;
# use std::fmt::Formatter;
pub struct ConfigurationError {
    backtrace: Backtrace,
}

impl ConfigurationError {
    pub(crate) fn new() -> Self {
        Self { backtrace: Backtrace::capture() }
    }
}

// Impl Debug + Display
```

Where appropriate, error types should provide contextual error information, for example:

```rust,ignore
# use std::backtrace::Backtrace;
# #[derive(Debug)]
# pub struct ConfigurationError {
#    backtrace: Backtrace,
# }
impl ConfigurationError {
    pub fn config_file(&self) -> &Path { }
}
```

If your API does mixed operations, or depends on various upstream libraries, store an `ErrorKind`.
Error kinds, and more generally enum-based errors, should not be used to avoid creating separate public error types when there is otherwise no error overlap:

```rust, ignore
// Prefer this
fn download_iso() -> Result<(), DownloadError> {}
fn start_vm() -> Result<(), VmError> {}

// Over that
fn download_iso() -> Result<(), GlobalEverythingErrorEnum> {}
fn start_vm() -> Result<(), GlobalEverythingErrorEnum> {}

// However, not every function warrants a new error type. Errors
// should be general enough to be reused.
fn parse_json() -> Result<(), ParseError> {}
fn parse_toml() -> Result<(), ParseError> {}
```

If you do use an inner `ErrorKind`, that enum should not be exposed directly for future-proofing reasons,
as otherwise you would expose your callers to _all_ possible failure modes, even the ones you consider internal
and unhandleable. Instead, expose various `is_xxx()` methods as shown below:

```rust
# use std::backtrace::Backtrace;
# use std::fmt::Display;
# use std::fmt::Formatter;
#[derive(Debug)]
pub(crate) enum ErrorKind {
    Io(std::io::Error),
    Protocol
}

#[derive(Debug)]
pub struct HttpError {
    kind: ErrorKind,
    backtrace: Backtrace,
}

impl HttpError {
    pub fn is_io(&self) -> bool { matches!(self.kind, ErrorKind::Io(_)) }
    pub fn is_protocol(&self) -> bool { matches!(self.kind, ErrorKind::Protocol) }
}
```

Most upstream errors don't provide a backtrace. You should capture one when creating an `Error` instance, either via one of
your `Error::new()` flavors, or when implementing `From<UpstreamError> for Error {}`.

Error structs must properly implement `Display` that renders as follows:

```rust,ignore
impl Display for MyError {
    // Print a summary sentence what happened.
    // Print `self.backtrace`.
    // Print any additional upstream 'cause' information you might have.
#   fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
#       todo!()
#   }
}
```

Errors must also implement `std::error::Error`:

```rust,ignore
impl std::error::Error for MyError { }
```

Lastly, if you happen to emit lots of errors from your crate, consider creating a private `bail!()` helper macro to simplify error instantiation.

> ### <tip></tip> When You Get Backtraces
>
> Backtraces are an invaluable debug tool in complex or async code, since  errors might _travel_ far through a callstack before being surfaced.
>
> That said, they are a _development_ tool, not a _runtime_ diagnostic, and by default `Backtrace::capture()` will **not** capture
> backtraces, as they have a large overhead, e.g., 4μs per capture on the author's PC.
>
> Instead, Rust evaluates a [set of environment variables](https://doc.rust-lang.org/stable/std/backtrace/index.html#environment-variables), such as
> `RUST_BACKTRACE`, and only walks the call frame when explicitly asked. Otherwise it captures an empty trace, at the cost of only a few CPU instructions.



## Essential Functionality Should be Inherent (M-ESSENTIAL-FN-INHERENT) { #M-ESSENTIAL-FN-INHERENT }

<why>To make essential functionality easily discoverable.</why>
<version>1.0</version>

Types should implement core functionality inherently. Trait implementations should forward to inherent functions, and not replace them. Instead of this

```rust
# trait Download {
#     fn download_file(&self, url: impl AsRef<str>);
# }
struct HttpClient {}

// Offloading essential functionality into traits means users
// will have to figure out what other traits to `use` to
// actually use this type.
impl Download for HttpClient {
    fn download_file(&self, url: impl AsRef<str>) {
        // ... logic to download a file
    }
}
```

do this:

```rust
# trait Download {
#     fn download_file(&self, url: impl AsRef<str>);
# }
struct HttpClient {}

impl HttpClient {
    fn download_file(&self, url: impl AsRef<str>) {
        // ... logic to download a file
    }
}

// Forward calls to inherent impls. `HttpClient` can be used
impl Download for HttpClient {
    fn download_file(&self, url: impl AsRef<str>) {
        Self::download_file(self, url)
    }
}
```



## Accept `impl AsRef<>` Where Feasible (M-IMPL-ASREF) { #M-IMPL-ASREF }

<why>To give users flexibility calling in with their own types.</why>
<version>1.0</version>

In **function** signatures, accept `impl AsRef<T>` for types that have a
[clear reference hierarchy](https://doc.rust-lang.org/stable/std/convert/trait.AsRef.html#implementors), where you
do not need to take ownership, or where object creation is relatively cheap.

| Instead of ... | accept ... |
| --- | --- |
| `&str`, `String` | `impl AsRef<str>` |
| `&Path`, `PathBuf` | `impl AsRef<Path>` |
| `&[u8]`, `Vec<u8>` | `impl AsRef<[u8]>` |

```rust,ignore
# use std::path::Path;
// Definitely use `AsRef`, the function does not need ownership.
fn print(x: impl AsRef<str>) {}
fn read_file(x: impl AsRef<Path>) {}
fn send_network(x: impl AsRef<[u8]>) {}

// Further analysis needed. In these cases the function wants
// ownership of some `String` or `Vec<u8>`. If those are
// "low freqency, low volume" functions `AsRef` has better ergonomics,
// otherwise accepting a `String` or `Vec<u8>` will have better
// performance.
fn new_instance(x: impl AsRef<str>) -> HoldsString {}
fn send_to_other_thread(x: impl AsRef<[u8]>) {}
```

In contrast, **types** should generally not be infected by these bounds:

```rust,ignore
// Generally not ok. There might be exceptions for performance
// reasons, but those should not be user visible.
struct User<T: AsRef<str>> {
    name: T
}

// Better
struct User {
    name: String
}
```



## Accept `impl 'IO'` Where Feasible ('Sans IO') (M-IMPL-IO) { #M-IMPL-IO }

<why>To untangle business logic from I/O logic, and have N*M composability.</why>
<version>0.1</version>

Functions and types that only need to perform one-shot I/O during initialization should be written "[sans-io](https://www.firezone.dev/blog/sans-io)",
and accept some `impl T`, where `T` is the appropriate I/O trait, effectively outsourcing I/O work to another type:

```rust,ignore
// Bad, caller must provide a File to parse the given data. If this
// data comes from the network, it'd have to be written to disk first.
fn parse_data(file: File) {}
```

```rust
// Much better, accepts
// - Files,
// - TcpStreams,
// - Stdin,
// - &[u8],
// - UnixStreams,
// ... and many more.
fn parse_data(data: impl std::io::Read) {}
```

Synchronous functions should use [`std::io::Read`](https://doc.rust-lang.org/std/io/trait.Read.html) and
[`std::io::Write`](https://doc.rust-lang.org/std/io/trait.Write.html). Asynchronous _functions_ targeting more than one runtime should use
[`futures::io::AsyncRead`](https://docs.rs/futures/latest/futures/io/trait.AsyncRead.html) and similar.
_Types_ that need to perform runtime-specific, continuous I/O should follow [M-RUNTIME-ABSTRACTED] instead.

[M-RUNTIME-ABSTRACTED]: ./#M-RUNTIME-ABSTRACTED



## Accept `impl RangeBounds<>` Where Feasible (M-IMPL-RANGEBOUNDS) { #M-IMPL-RANGEBOUNDS }

<why>To give users flexibility and clarity when specifying ranges.</why>
<version>1.0</version>

Functions that accept a range of numbers must use a `Range` type or trait over hand-rolled parameters:

```rust,ignore
// Bad
fn select_range(low: usize, high: usize) {}
fn select_range(range: (usize, usize)) {}
```

In addition, functions that can work on arbitrary ranges, should accept `impl RangeBounds<T>` rather than `Range<T>`.

```rust
# use std::ops::{RangeBounds, Range};
// Callers must call with `select_range(1..3)`
fn select_range(r: Range<usize>) {}

// Callers may call as
//     select_any(1..3)
//     select_any(1..)
//     select_any(..)
fn select_any(r: impl RangeBounds<usize>) {}
```



## Complex Type Construction has Builders (M-INIT-BUILDER) { #M-INIT-BUILDER }

<why>To future-proof type construction in complex scenarios.</why>
<version>0.3</version>

Types that could support 4 or more arbitrary initialization permutations should provide builders. In other words, types with up to
2 optional initialization parameters can be constructed via inherent methods:

```rust
# struct A;
# struct B;
struct Foo;

// Supports 2 optional construction parameters, inherent methods ok.
impl Foo {
    pub fn new() -> Self { Self }
    pub fn with_a(a: A) -> Self { Self }
    pub fn with_b(b: B) -> Self { Self }
    pub fn with_a_b(a: A, b: B) -> Self { Self }
}
```

Beyond that, types should provide a builder:

```rust, ignore
# struct A;
# struct B;
# struct C;
# struct Foo;
# struct FooBuilder;
impl Foo {
    pub fn new() -> Self { ... }
    pub fn builder() -> FooBuilder { ... }
}

impl FooBuilder {
    pub fn a(mut self, a: A) -> Self { ... }
    pub fn b(mut self, b: B) -> Self { ... }
    pub fn c(mut self, c: C) -> Self { ... }
    pub fn build(self) -> Foo { ... }
}

```

The proper name for a builder that builds `Foo` is `FooBuilder`. Its methods must be chainable, with the final method called
`.build()`. The buildable struct must have a shortcut `Foo::builder()`, while the builder itself should _not_ have a public
`FooBuilder::new()`. Builder methods that set a value `x` are called `x()`, not `set_x()` or similar.

### Builders and Required Parameters

Required parameters should be passed when creating the builder, not as setter methods. For builders with multiple required
parameters, encapsulate them into a parameters struct and use the `deps: impl Into<Deps>` pattern to provide flexibility:

> **Note:** A dedicated deps struct is not required if the builder has no required parameters or only a single simple parameter. However,
> for backward compatibility and API evolution, it's preferable to use a dedicated struct for deps even in simple cases, as it makes it
> easier to add new required parameters in the future without breaking existing code.

```rust, ignore
#[derive(Debug, Clone)]
pub struct FooDeps {
    pub logger: Logger,
    pub config: Config,
}

impl From<(Logger, Config)> for FooDeps { ... }
impl From<Logger> for FooDeps { ... } // In case we could use default Config instance

impl Foo {
    pub fn builder(deps: impl Into<FooDeps>) -> FooBuilder { ... }
}
```

This pattern allows for convenient usage:

- `Foo::builder(logger)` - when only the logger is needed
- `Foo::builder((logger, config))` - when both parameters are needed
- `Foo::builder(FooDeps { logger, config })` - explicit struct construction

Alternatively, you can use [`fundle`](https://docs.rs/fundle) to simplify the creation of `FooDeps`:

```rust, ignore
#[derive(Debug, Clone)]
#[fundle::deps]
pub struct FooDeps {
    pub logger: Logger,
    pub config: Config,
}
```

This pattern enables "dependency injection", see [these docs](https://docs.rs/fundle/latest/fundle/attr.deps.html) for more details.

### Runtime-Specific Builders

For types that are runtime-specific or require runtime-specific configuration, provide dedicated builder creation methods that accept the appropriate runtime parameters:

```rust, ignore
#[cfg(feature="smol")]
#[derive(Debug, Clone)]
pub struct SmolDeps {
    pub clock: Clock,
    pub io_context: Context,
}

#[cfg(feature="tokio")]
#[derive(Debug, Clone)]
pub struct TokioDeps {
    pub clock: Clock,
}

impl Foo {
    #[cfg(feature="smol")]
    pub fn builder_smol(deps: impl Into<SmolDeps>) -> FooBuilder { ... }

    #[cfg(feature="tokio")]
    pub fn builder_tokio(deps: impl Into<TokioDeps>) -> FooBuilder { ... }
}
```

This approach ensures type safety at compile time and makes the runtime dependency explicit in the API surface. The resulting
builder methods follow the pattern `builder_{runtime}(deps)` where `{runtime}` indicates the specific runtime or execution environment.

### Further Reading

- [Builder pattern in Rust: self vs. &mut self, and method vs. associated function](https://users.rust-lang.org/t/builder-pattern-in-rust-self-vs-mut-self-and-method-vs-associated-function/72892)
- [fundle](https://docs.rs/fundle)



## Complex Type Initialization Hierarchies are Cascaded (M-INIT-CASCADED) { #M-INIT-CASCADED }

<why>To prevent misuse and accidental parameter mix ups.</why>
<version>1.0</version>

Types that require 4+ parameters should cascade their initialization via helper types.

```rust, ignore
# struct Deposit;
impl Deposit {
    // Easy to confuse parameters and signature generally unwieldy.
    pub fn new(bank_name: &str, customer_name: &str, currency_name: &str, currency_amount: u64) -> Self { }
}
```

Instead of providing a long parameter list, parameters should be grouped semantically. When applying this guideline,
also check if [C-NEWTYPE] is applicable:

```rust, ignore
# struct Deposit;
# struct Account;
# struct Currency
impl Deposit {
    // Better, signature cleaner
    pub fn new(account: Account, amount: Currency) -> Self { }
}

impl Account {
    pub fn new_ok(bank: &str, customer: &str) -> Self { }
    pub fn new_even_better(bank: Bank, customer: Customer) -> Self { }
}
```

[C-NEWTYPE]: https://rust-lang.github.io/api-guidelines/type-safety.html#c-newtype



## Services are Clone (M-SERVICES-CLONE) { #M-SERVICES-CLONE }

<why>To avoid composability issues when sharing common services.</why>
<version>1.0</version>

Heavyweight _service_ types and 'thread singletons' should implement shared-ownership `Clone` semantics, including any type you expect to be used from your `Application::init`.

Per thread, users should essentially be able to create a single resource handler instance, and have it reused by other handlers on the same thread:

```rust,ignore
impl ThreadLocal for MyThreadState {
    fn init(...) -> Self {

        // Create common service instance possibly used by many.
        let common = ServiceCommon::new();

        // Users can freely pass `common` here multiple times
        let service_1 = ServiceA::new(&common);
        let service_2 = ServiceA::new(&common);

        Self { ... }
    }
}
```

Services then simply clone their dependency and store a new _handle_, as if `ServiceCommon` were a shared-ownership smart pointer:

```rust,ignore
impl ServiceA {
    pub fn new(common: &ServiceCommon) -> Self {
        // If we only need to access `common` from `new` we don't have
        // to store it. Otherwise, make a clone we store in `Self`.
        let common = common.clone();
    }
}
```

Under the hood this `Clone` should **not** create a fat copy of the entire service. Instead, it should follow the `Arc<Inner>` pattern:

```rust, ignore
// Actual service containing core logic and data.
struct ServiceCommonInner {}

#[derive(Clone)]
pub ServiceCommon {
    inner: Arc<ServiceCommonInner>
}

impl ServiceCommon {
    pub fn new() {
        Self { inner: Arc::new(ServiceCommonInner::new()) }
    }

    // Method forwards ...
    pub fn foo(&self) { self.inner.foo() }
    pub fn bar(&self) { self.inner.bar() }
}
```



## Abstractions Don't Visibly Nest (M-SIMPLE-ABSTRACTIONS) { #M-SIMPLE-ABSTRACTIONS }

<why>To prevent cognitive load and a bad out of the box UX.</why>
<version>0.1</version>

When designing your public types and primary API surface, avoid exposing nested or complex parametrized types to your users.

While powerful, type parameters introduce a cognitive load, even more so if the involved traits are crate-specific. Type parameters
become infectious to user code holding on to these types in their fields, often come with complex trait hierarchies on their own, and
might cause confusing error messages.

From the perspective of a user authoring `Foo`, where the other structs come from your crate:

```rust,ignore
struct Foo {
    service: Service // Great
    service: Service<Backend> // Acceptable
    service: Service<Backend<Store>> // Bad

    list: List<Rc<u32>> // Great, `List<T>` is simple container,
                        // other types user provided.

    matrix: Matrix4x4 // Great
    matrix: Matrix4x4<f32> // Still ok
    matrix: Matrix<f32, Const<4>, Const<4>, ArrayStorage<f32, 4, 4>> // ?!?
}
```

_Visible_ type parameters should be avoided in _service-like_ types (i.e., types mainly instantiated once per thread / application that are often passed
as dependencies), in particular if the nestee originates from the same crate as the service.

Containers, smart-pointers and similar data structures obviously must expose a type parameter, e.g., `List<T>` above. Even then, care should
be taken to limit the number and nesting of parameters.

To decide whether type parameter nesting should be avoided, consider these factors:

- Will the type be **named** by your users?
  - Service-level types are always expected to be named (e.g., `Library<T>`),
  - Utility types, such as the many [`std::iter`](https://doc.rust-lang.org/stable/std/iter/index.html) types like `Chain`, `Cloned`, `Cycle`, are not
    expected to be named.
- Does the type primarily compose with non-user types?
- Do the used type parameters have complex bounds?
- Do the used type parameters affect inference in other types or functions?

The more of these factors apply, the bigger the cognitive burden.

As a rule of thumb, primary service API types should not nest _on their own volition_, and if they do, only 1 level deep. In other words, these
APIs should not require users having to deal with an `Foo<Bar<FooBar>>`. However, if `Foo<T>` users want to bring their own `A<B<C>>` as `T` they
should be free to do so.

> ### <tip></tip> Type Magic for Better UX?
>
> The guideline above is written with 'bread-and-butter' types in mind you might create during  _normal_ development activity. Its intention is to
> reduce friction users encounter when working with your code.
>
> However, when designing API patterns and ecosystems at large, there might be valid reasons to introduce intricate type magic to overall _lower_
> the cognitive friction involved, [Bevy's ECS](https://docs.rs/bevy_ecs/latest/bevy_ecs/) or
> [Axum's request handlers](https://docs.rs/axum/latest/axum/handler/trait.Handler.html) come to mind.
>
> The threshold where this pays off is high though. If there is any doubt about the utility of your creative use of generics, your users might be
> better off without them.



