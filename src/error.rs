use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Debug)]
pub enum CompilerError {
    Io(std::io::Error),
    InvalidArguments(String),
    Lex(String),
    Parse(String),
    Semantic(String),
    Codegen(String),
}

impl Display for CompilerError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::InvalidArguments(message) => write!(formatter, "Argument error: {message}"),
            Self::Lex(message) => write!(formatter, "Lex error: {message}"),
            Self::Parse(message) => write!(formatter, "Parse error: {message}"),
            Self::Semantic(message) => write!(formatter, "Semantic error: {message}"),
            Self::Codegen(message) => write!(formatter, "Code generation error: {message}"),
        }
    }
}

impl Error for CompilerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for CompilerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

pub type CompilerResult<T> = Result<T, CompilerError>;
