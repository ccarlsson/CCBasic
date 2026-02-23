# Software Requirement Specification (SRS)

## Project: MBASIC-R Compiler

------------------------------------------------------------------------

## 1. Introduction

The MBASIC-R Compiler is a Rust-based compiler that translates a defined
subset of Microsoft BASIC into native Linux ELF x86_64 executables.\
The project is designed for educational purposes to explore compiler
architecture, static analysis, and low-level code generation.

------------------------------------------------------------------------

## 2. Scope

The compiler supports a restricted BASIC dialect called **MBASIC-R
(v0.1)**.

The compiler shall: - Parse line-numbered BASIC source files - Perform
lexical, syntactic, and semantic analysis - Generate x86_64 assembly
(NASM-compatible) - Produce Linux ELF executables

------------------------------------------------------------------------

## 3. Language Definition (MBASIC-R v0.1)

### 3.1 General Characteristics

-   Line-numbered program structure
-   Single statement per line
-   Case insensitive
-   Integer variables only (A--Z)
-   Signed 64-bit integers
-   Compiled (no interpreter)

------------------------------------------------------------------------

### 3.2 Supported Statements

-   `LET`
-   `PRINT`
-   `GOTO`
-   `IF ... THEN`
-   `END`
-   `REM`

------------------------------------------------------------------------

### 3.3 Expressions

Supported:

-   Integer literals
-   Variables (A--Z)
-   Operators: `+ - * /`
-   Comparison: `= <> < > <= >=`
-   Parentheses

------------------------------------------------------------------------

## 4. Formal Grammar (EBNF)

    program         = { line } EOF ;

    line            = line_number statement NEWLINE ;

    statement       =
          let_stmt
        | print_stmt
        | goto_stmt
        | if_stmt
        | end_stmt
        | rem_stmt ;

    let_stmt        = "LET" variable "=" expression ;

    print_stmt      = "PRINT" print_item { "," print_item } ;

    goto_stmt       = "GOTO" line_number ;

    if_stmt         = "IF" condition "THEN" line_number ;

    end_stmt        = "END" ;

------------------------------------------------------------------------

## 5. Functional Requirements

The system shall:

1.  Parse valid MBASIC-R source files.
2.  Validate unique line numbers.
3.  Validate variable initialization before use.
4.  Validate GOTO and IF targets exist.
5.  Generate valid x86_64 assembly.
6.  Produce runnable Linux ELF executables.
7.  Return meaningful error messages on failure.

------------------------------------------------------------------------

## 6. Non-Functional Requirements

-   Implementation language: Rust
-   Target platform: Linux x86_64
-   Deterministic compilation
-   Modular architecture
-   No unsafe Rust unless strictly necessary

------------------------------------------------------------------------

## 7. Compiler Architecture

    Source (.bas)
        ↓
    Lexer
        ↓
    Parser
        ↓
    AST
        ↓
    Semantic Analyzer
        ↓
    Code Generator (x86_64)
        ↓
    Linux ELF Executable

------------------------------------------------------------------------

## 8. Future Extensions

-   FOR/NEXT loops
-   INPUT statement
-   DIM arrays
-   GOSUB/RETURN
-   String type support
-   Intermediate Representation (IR)
-   Register allocation improvements
