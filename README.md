# MBASIC-R Compiler

MBASIC-R is a Rust compiler for a restricted Microsoft BASIC subset that targets Linux x86_64.
It compiles line-numbered `.bas` programs into NASM-compatible assembly and optionally into runnable ELF executables.

## Disclaimer

This repository is a personal learning project for studying compiler construction and Rust.
It is built for educational purposes and experimentation, not for production use.

## Project Layout

- `src/main.rs` - CLI and compile pipeline orchestration
- `src/lexer.rs` - lexical analysis
- `src/parser.rs` - parser + AST construction
- `src/ast.rs` - AST definitions
- `src/semantic.rs` - semantic checks and definite-assignment analysis
- `src/codegen/x86_64.rs` - NASM x86_64 code generation
- `tests/` - BASIC fixtures and integration tests

## Prerequisites

- Rust toolchain (`cargo`, `rustc`)
- Linux x86_64
- `nasm` and `ld` available on `PATH` for executable generation and e2e tests

## Build

```bash
cargo build
```

## Run

### Emit assembly only

```bash
cargo run -- program.bas --emit-asm-only --asm-out program.asm
```

### Build executable (default mode)

```bash
cargo run -- program.bas -o program
./program
```

### Build executable and keep emitted assembly

```bash
cargo run -- program.bas --emit-asm --asm-out program.asm -o program
```

## CLI

```text
mbasicr <input.bas> [-o <out>] [--emit-asm] [--emit-asm-only] [--asm-out <file.asm>] [--keep-asm]
```

Behavior summary:

- Default: builds ELF executable (`nasm` + `ld`) and uses a temporary `.asm`.
- `--emit-asm-only`: writes assembly only (no link step).
- `--emit-asm`: builds ELF and also persists assembly output.
- `--keep-asm`: valid with default ELF mode to keep generated `.asm`.

## Language Notes (Current)

- Variables:
	- Integer: `A`..`Z`
	- String: `A$`..`Z$`
- Supported statements: `LET`, `PRINT`, `INPUT`, `GOTO`, `IF ... THEN`, `END`, `REM`
- `+` is overloaded:
	- `int + int` performs integer addition
	- `str + str` performs string concatenation
	- mixed string/int operands are rejected by semantic analysis
- String literals use double quotes. To include a quote inside a string, use doubled quotes:
	- Example: `"HE SAID ""HI"""` produces `HE SAID "HI"`

## Tests

### Unit + integration

```bash
cargo test
```

This runs:

- lexer, parser, semantic, and codegen unit tests
- `tests/e2e.rs` integration test that performs compile → assemble → link → execute and validates stdout against fixture outputs

### Included BASIC fixtures

- `tests/print_arith.bas` → `tests/print_arith.out`
- `tests/if_goto.bas` → `tests/if_goto.out`
- `tests/print_multi.bas` → `tests/print_multi.out`
- `tests/print_string.bas` → `tests/print_string.out`
- `tests/print_escaped_quote.bas` → `tests/print_escaped_quote.out`
- `tests/concat.bas` → `tests/concat.out`
- `tests/input_int.bas` + `tests/input_int.in` → `tests/input_int.out`
- `tests/input_str.bas` + `tests/input_str.in` → `tests/input_str.out`
