# GitHub Copilot Instruction Document

## Project: MBASIC-R Compiler

------------------------------------------------------------------------

## 1. Project Context

MBASIC-R is a Rust-based compiler that compiles a restricted Microsoft
BASIC subset into Linux x86_64 ELF executables.

Architecture must remain modular and deterministic.

------------------------------------------------------------------------

## 2. Required Module Structure

    src/
     ├── main.rs
     ├── lexer.rs
     ├── parser.rs
     ├── ast.rs
     ├── semantic.rs
     └── codegen/
           └── x86_64.rs

------------------------------------------------------------------------

## 3. Coding Standards

-   Use idiomatic Rust
-   Avoid global mutable state
-   Prefer enums over trait objects for AST
-   Use `Result<T, CompilerError>` for fallible functions
-   No `unsafe` unless explicitly required
-   Write unit tests for lexer and parser

------------------------------------------------------------------------

## 4. AST Requirements

-   Program stored as `BTreeMap<u32, Statement>`
-   AST must be immutable after construction
-   Expressions represented as enum tree structures

------------------------------------------------------------------------

## 5. Code Generation Rules

-   Generate NASM-compatible assembly
-   Map each BASIC line number to a unique assembly label
-   Use Linux syscalls for `write` and `exit`
-   Preserve evaluation order
-   No hidden runtime dependencies

------------------------------------------------------------------------

## 6. Testing Requirements

-   Provide BASIC examples in `/tests`
-   Automate compile → assemble → run → validate stdout
-   Ensure deterministic builds
-   Fail fast on semantic errors

------------------------------------------------------------------------

## 7. Copilot Guidance

When generating code:

-   Do not introduce additional language features
-   Follow the defined grammar strictly
-   Do not change architecture
-   Maintain separation of concerns
-   Keep functions small and testable
