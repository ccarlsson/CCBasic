## Plan: MBASIC-R v0.1 Implementation (Scaffold → Compile → Run)

**Status (2026-02-23):** Steps 1 through 7 are implemented and validated (`cargo test` passes, including e2e compile → assemble → run checks).

This repo currently has only the SRS and Copilot instructions, so the first milestone is scaffolding a Cargo/Rust compiler project that matches the required module layout, then implementing lexer → parser/AST → semantic analysis → NASM x86_64 codegen, plus unit tests and an end-to-end harness that compiles `.bas` → assembles/links → runs → checks stdout. Decisions already locked in: emit `.asm` and optionally build an ELF; `PRINT` joins items with one space and ends with newline; `REM` ignores rest of line; integer division truncates toward zero.

Save this plan as: [docs/MBASIC-R_Implementation_Plan.md](docs/MBASIC-R_Implementation_Plan.md)

**Steps**
1. Repo scaffold + CLI shape
   1. Create a Cargo binary crate at repo root and add required modules:
      - [src/main.rs](src/main.rs)
      - [src/lexer.rs](src/lexer.rs)
      - [src/parser.rs](src/parser.rs)
      - [src/ast.rs](src/ast.rs)
      - [src/semantic.rs](src/semantic.rs)
      - [src/codegen/x86_64.rs](src/codegen/x86_64.rs)
   2. Add `CompilerError` used everywhere via `Result<T, CompilerError>` (either in [src/main.rs](src/main.rs) or [src/error.rs](src/error.rs) if you prefer a dedicated module).
   3. CLI (minimal, deterministic defaults):
      - Usage: `mbasicr <input.bas> [-o <out>] [--emit-asm] [--asm-out <file.asm>] [--keep-asm]`
      - Defaults:
         - If `--emit-asm` is set and no build requested: write `.asm` to `--asm-out` or `<out>.asm`
         - If building ELF (default unless `--emit-asm`-only is chosen): produce executable at `-o <out>` (default: input stem), and generate a temporary `.asm` unless `--keep-asm` is set
      - Tooling assumptions: `nasm` and `ld` available on PATH for ELF builds; if missing, fail with a clear error message.

2. AST design (immutable after construction; program is a `BTreeMap`)
   1. In [src/ast.rs](src/ast.rs), define:
      - `Program(BTreeMap<u32, Statement>)`
      - `Statement` enum: `Let`, `Print`, `Goto`, `IfThen`, `End`, `Rem`
      - `Expr` enum: `Int(i64)`, `Var(Var)`, `Binary { op, left, right }` (no mutation)
      - `Var` constrained to A–Z (e.g., `u8` index 0..25)
      - `BinOp`: `Add/Sub/Mul/Div`
      - `CmpOp`: `Eq/Ne/Lt/Gt/Le/Ge`
   2. Keep parsing output immutable: no later AST rewriting passes.

3. Lexer (case-insensitive; line/column spans)
   1. In [src/lexer.rs](src/lexer.rs), tokenize:
      - Keywords: `LET PRINT GOTO IF THEN END REM` (case-insensitive)
      - Line numbers: unsigned integer at start of each line
      - Variables: single letter `A`–`Z`
      - Literals: signed 64-bit integers (support unary `-` as part of parsing, or lex a `-` token and handle unary in parser)
      - Operators/punct: `+ - * / = <> < > <= >= ( ) ,`
      - `NEWLINE`, `EOF`
   2. `REM` rule: when `REM` is encountered as a statement keyword, consume/ignore the rest of the line and emit `NEWLINE` next.
   3. Provide spans for meaningful diagnostics (line, column, maybe byte offset).

4. Parser (single statement per line + expression precedence)
   1. In [src/parser.rs](src/parser.rs), implement:
      - `parse_program(tokens) -> Result<Program, CompilerError>`
      - Enforce “one statement per line” by requiring `NEWLINE` after each statement.
      - Reject duplicate line numbers while building the `BTreeMap<u32, Statement>`.
   2. Parse statements (per SRS intent, filling in missing EBNF details consistently):
      - `LET <var> = <expr>`
      - `PRINT <expr> (, <expr>)*`
      - `GOTO <line_number>`
      - `IF <expr> <cmp> <expr> THEN <line_number>`
      - `END`
      - `REM <ignored…>`
   3. Expression parsing:
      - Precedence: `* /` > `+ -`
      - Parentheses supported
      - Preserve left-to-right evaluation (important later in codegen)

5. Semantic analysis (fail fast; targets exist; definite assignment)
   1. In [src/semantic.rs](src/semantic.rs):
      - Validate all `GOTO` and `IF ... THEN` targets exist.
      - Validate variable initialization before any read using control-flow-aware definite assignment:
         - Build a CFG over line numbers (edges: fallthrough to next line, plus jump edges for `GOTO` and `IF`, no outgoing from `END`)
         - Run a forward must-analysis with intersection at joins
         - Check every variable read in expressions against the “definitely initialized” set at that program point
      - Optional but helpful: flag unreachable code (not required by SRS; skip unless you want it).

6. NASM x86_64 codegen (+ optional assemble/link)
   1. In [src/codegen/x86_64.rs](src/codegen/x86_64.rs), emit NASM-compatible assembly:
      - Deterministic label scheme: `line_<N>` for BASIC line `N`
      - Variables A–Z in `.bss` as 26 qwords
      - Linux syscalls only (`write`, `exit`)
   2. Statement codegen:
      - `LET`: eval expr → store to var slot
      - `PRINT`: eval items left-to-right; print signed decimal; insert single space between items; newline at end
      - `GOTO`: `jmp line_<target>`
      - `IF`: eval LHS/RHS; `cmp`; conditional jump to target; otherwise fallthrough
      - `END`: syscall `exit(0)`
      - `REM`: no-op
   3. Expression codegen strategy (simple and correct first):
      - Stack-based evaluation with `push`/`pop` to preserve evaluation order
      - Signed division via `cqo` + `idiv` (trunc toward zero)
   4. Printing helper routine:
      - Emit an internal `print_i64` routine that converts i64 to ASCII into a buffer and calls `write(1, buf, len)`
      - Keep it fully self-contained in generated assembly (still “no runtime deps”)

7. Testing (unit tests + end-to-end `.bas` examples)
   1. Unit tests (required) in:
      - [src/lexer.rs](src/lexer.rs): keywords case-insensitivity; `<=` and `<>`; newline handling; `REM` consumes rest of line
      - [src/parser.rs](src/parser.rs): precedence; `IF ... THEN`; duplicate line numbers rejected
   2. End-to-end examples in `/tests` (required) plus a harness that compiles/assembles/links/runs and validates stdout.
      - Example program 1: `tests/print_arith.bas`
         - Lines (conceptual): `10 LET A = 40 + 2`, `20 PRINT A`, `30 END`
         - Expected stdout: `42\n`
      - Example program 2: `tests/if_goto.bas`
         - Lines: init `A`, conditional branch via `IF A = 1 THEN 50`, print different values, ensure only correct path prints
         - Expected stdout: either `100\n` or `200\n` depending on setup (pick one deterministic setup, e.g., `A=1` so it prints `200\n`)
      - Example program 3: `tests/print_multi.bas`
         - Lines: `10 LET A = 7`, `20 PRINT A, 8, 9`, `30 END`
         - Expected stdout: `7 8 9\n`
      - Harness behavior:
         - Build compiler once, then for each `.bas`: run compiler → run `nasm` → run `ld` → execute ELF → compare stdout

**Verification**
- `cargo test` (lexer/parser unit tests and end-to-end integration tests)
- Manual smoke:
   - `cargo run -- tests/print_arith.bas --emit-asm --asm-out out.asm` then `nasm -felf64 out.asm -o out.o && ld out.o -o out && ./out`
- Determinism check: compile same `.bas` twice and diff `.asm` output.

**Decisions**
- Output: emit `.asm` and optionally build ELF via `nasm`+`ld`
- `PRINT`: join items with single spaces, newline at end
- `REM`: ignore rest of line
- Division semantics: truncate toward zero
