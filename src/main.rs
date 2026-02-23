mod ast;
pub mod codegen {
    pub mod x86_64;
}
mod error;
mod lexer;
mod parser;
mod semantic;

use std::env;
use std::path::PathBuf;
use std::process;

use error::{CompilerError, CompilerResult};

#[derive(Debug, Clone)]
pub struct CliOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub emit_asm: bool,
    pub asm_out: Option<PathBuf>,
    pub keep_asm: bool,
}

impl CliOptions {
    fn usage() -> &'static str {
        "Usage: mbasicr <input.bas> [-o <out>] [--emit-asm] [--asm-out <file.asm>] [--keep-asm]"
    }

    pub fn parse_from(args: impl IntoIterator<Item = String>) -> CompilerResult<Self> {
        let mut iter = args.into_iter();
        let mut input: Option<PathBuf> = None;
        let mut output: Option<PathBuf> = None;
        let mut emit_asm = false;
        let mut asm_out: Option<PathBuf> = None;
        let mut keep_asm = false;

        while let Some(argument) = iter.next() {
            match argument.as_str() {
                "-h" | "--help" => {
                    return Err(CompilerError::InvalidArguments(Self::usage().to_string()));
                }
                "-o" => {
                    let value = iter.next().ok_or_else(|| {
                        CompilerError::InvalidArguments("Missing value after -o".to_string())
                    })?;
                    output = Some(PathBuf::from(value));
                }
                "--emit-asm" => {
                    emit_asm = true;
                }
                "--asm-out" => {
                    let value = iter.next().ok_or_else(|| {
                        CompilerError::InvalidArguments("Missing value after --asm-out".to_string())
                    })?;
                    asm_out = Some(PathBuf::from(value));
                }
                "--keep-asm" => {
                    keep_asm = true;
                }
                value if value.starts_with('-') => {
                    return Err(CompilerError::InvalidArguments(format!(
                        "Unknown flag: {value}"
                    )));
                }
                value => {
                    if input.is_some() {
                        return Err(CompilerError::InvalidArguments(
                            "Only one input file is supported".to_string(),
                        ));
                    }
                    input = Some(PathBuf::from(value));
                }
            }
        }

        let input = input.ok_or_else(|| {
            CompilerError::InvalidArguments(format!("{}", Self::usage()))
        })?;

        Ok(Self {
            input,
            output,
            emit_asm,
            asm_out,
            keep_asm,
        })
    }
}

fn run() -> CompilerResult<()> {
    let _options = CliOptions::parse_from(env::args().skip(1))?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}
