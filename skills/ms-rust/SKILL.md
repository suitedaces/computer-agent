---
name: ms-rust
description: ALWAYS invoke this skill BEFORE writing or modifying ANY Rust code (.rs files) even for simple Hello World programs. Enforces Microsoft-style Rust development discipline and requires consulting the appropriate guideline files before any coding activity. This skill is MANDATORY for all Rust development.
---

# Rust Development Skill

This skill enforces structured, guideline-driven Rust development following Microsoft's Pragmatic Rust Guidelines.

## Mandatory Workflow

**This skill MUST be invoked for ANY Rust action**, including:
- Creating new `.rs` files (even minimal examples like Hello World)
- Modifying existing `.rs` files (any change, no matter how small)
- Reviewing or refactoring Rust code

## Which guideline to read and when

Before writing or modifying Rust code, **Claude must load ONLY the guideline files that apply to the requested task**.

### Guidelines and when they apply

#### 1. [universal-guidelines](guidelines/universal-guidelines/SKILL.md)
**Use in ALL Rust tasks.** This file defines general Rust best practices, style, naming, logging, panics, Debug/Display implementations, static verification with clippy, and cross-cutting concerns that apply everywhere.

#### 2. [ai-guidelines](guidelines/ai-guidelines/SKILL.md)
Use when the Rust code involves AI agents, LLM-driven code generation, making APIs easier for AI systems to use, comprehensive documentation and detailed examples, or strong type systems that help AI avoid mistakes.

#### 3. [application-guidelines](guidelines/application-guidelines/SKILL.md)
Use when working on application-level error handling with anyhow or eyre, CLI tools and desktop applications, performance optimization using mimalloc allocator, or user-facing features and initialization logic.

#### 4. [documentation-guidelines](guidelines/documentation-guidelines/SKILL.md)
Use when writing public API documentation and doc comments, creating canonical documentation sections (Examples, Errors, Panics, Safety), structuring module-level documentation, or using re-exports with doc(inline) annotations.

#### 5. [ffi-guidelines](guidelines/ffi-guidelines/SKILL.md)
Use when loading multiple Rust-based dynamic libraries (DLLs), creating FFI boundaries and interoperability layers, sharing data between different Rust compilation artifacts, or dealing with portable vs non-portable data types across DLL boundaries.

#### 6. [performance-guidelines](guidelines/performance-guidelines/SKILL.md)
Use when identifying and profiling hot paths in your code, optimizing for throughput and CPU cycle efficiency, managing allocation patterns and memory usage, or implementing yield points in long-running async tasks.

#### 7. [safety-guidelines](guidelines/safety-guidelines/SKILL.md)
Use when writing unsafe code for novel abstractions, performance, or FFI, ensuring code soundness and preventing undefined behavior, documenting safety requirements and invariants, or reviewing unsafe blocks for correctness with Miri.

#### 8. [libraries-building](guidelines/libraries-building/SKILL.md)
Use when creating reusable library crates, managing Cargo features and their additivity, building native sys crates for C interoperability, or ensuring libraries work out-of-the-box on all platforms.

#### 9. [libraries-interoperability](guidelines/libraries-interoperability/SKILL.md)
Use when exposing public APIs and managing external dependencies, designing types for Send/Sync compatibility, avoiding leaking third-party types from public APIs, or creating escape hatches for native handle interop.

#### 10. [libraries-resilience](guidelines/libraries-resilience/SKILL.md)
Use when avoiding statics and thread-local state in libraries, making I/O and system calls mockable for testing, preventing glob re-exports and accidental leaks, or feature-gating test utilities and mocking functionality.

#### 11. [libraries-ux](guidelines/libraries-ux/SKILL.md)
Use when designing user-friendly library APIs, managing error types and error handling patterns, creating runtime abstractions and trait-based designs, implementing builders for complex initialization, or structuring crate organization.

## Coding Rules

1. **Load the necessary guideline files BEFORE ANY RUST CODE GENERATION.**
2. Apply the required rules from the relevant guidelines.
3. Apply the *M-CANONICAL-DOCS* documentation format for public items.
4. Comments must ALWAYS be written in American English.
5. If the file is fully compliant, add a comment: `// Rust guideline compliant YYYY-MM-DD`