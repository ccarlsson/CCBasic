mod ast;
pub mod codegen {
    pub mod x86_64;
}
mod error;
mod lexer;
mod parser;
mod semantic;

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process;

use error::{CompilerError, CompilerResult};

#[derive(Debug, Clone)]
pub struct CliOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub emit_asm: bool,
    pub emit_asm_only: bool,
    pub asm_out: Option<PathBuf>,
    pub keep_asm: bool,
}

#[derive(Debug, Clone)]
pub enum BuildMode {
    BuildElf,
    BuildElfAndEmitAsm,
    EmitAsmOnly,
}

#[derive(Debug, Clone)]
pub enum AsmArtifact {
    Temporary(PathBuf),
    Persisted(PathBuf),
}

#[derive(Debug, Clone)]
pub struct ResolvedCliOptions {
    pub input: PathBuf,
    pub mode: BuildMode,
    pub executable_out: Option<PathBuf>,
    pub asm_artifact: AsmArtifact,
}

impl CliOptions {
    fn usage() -> &'static str {
        "Usage: mbasicr <input.bas> [-o <out>] [--emit-asm] [--emit-asm-only] [--asm-out <file.asm>] [--keep-asm]"
    }

    pub fn parse_from(args: impl IntoIterator<Item = String>) -> CompilerResult<Self> {
        let mut iter = args.into_iter();
        let mut input: Option<PathBuf> = None;
        let mut output: Option<PathBuf> = None;
        let mut emit_asm = false;
        let mut emit_asm_only = false;
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
                "--emit-asm-only" => {
                    emit_asm_only = true;
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
            emit_asm_only,
            asm_out,
            keep_asm,
        })
    }

    pub fn resolve(self) -> CompilerResult<ResolvedCliOptions> {
        if self.emit_asm_only && self.keep_asm {
            return Err(CompilerError::InvalidArguments(
                "--keep-asm cannot be used with --emit-asm-only".to_string(),
            ));
        }

        let executable_default = Self::default_executable_path(&self.input)?;
        let executable_out = self.output.clone().unwrap_or(executable_default);

        let mode = if self.emit_asm_only {
            BuildMode::EmitAsmOnly
        } else if self.emit_asm {
            BuildMode::BuildElfAndEmitAsm
        } else {
            BuildMode::BuildElf
        };

        let asm_artifact = match mode {
            BuildMode::EmitAsmOnly | BuildMode::BuildElfAndEmitAsm => {
                AsmArtifact::Persisted(self.default_persisted_asm_path(&executable_out))
            }
            BuildMode::BuildElf => {
                if self.keep_asm {
                    AsmArtifact::Persisted(self.default_persisted_asm_path(&executable_out))
                } else {
                    AsmArtifact::Temporary(Self::default_temporary_asm_path(&executable_out))
                }
            }
        };

        let executable_out = match mode {
            BuildMode::EmitAsmOnly => None,
            _ => Some(executable_out),
        };

        Ok(ResolvedCliOptions {
            input: self.input,
            mode,
            executable_out,
            asm_artifact,
        })
    }

    fn default_executable_path(input: &PathBuf) -> CompilerResult<PathBuf> {
        let stem = input.file_stem().ok_or_else(|| {
            CompilerError::InvalidArguments(format!(
                "Input path '{}' does not contain a valid file stem",
                input.display()
            ))
        })?;

        Ok(PathBuf::from(stem))
    }

    fn default_temporary_asm_path(executable_out: &PathBuf) -> PathBuf {
        let mut value = executable_out.as_os_str().to_owned();
        value.push(".mbasicr.tmp.asm");
        PathBuf::from(value)
    }

    fn default_persisted_asm_path(&self, executable_out: &PathBuf) -> PathBuf {
        self.asm_out
            .clone()
            .unwrap_or_else(|| Self::with_suffix(executable_out, ".asm"))
    }

    fn with_suffix(path: &PathBuf, suffix: &str) -> PathBuf {
        let mut value: OsString = path.as_os_str().to_owned();
        value.push(suffix);
        PathBuf::from(value)
    }
}

fn run() -> CompilerResult<()> {
    let _resolved = CliOptions::parse_from(env::args().skip(1))?.resolve()?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}
