use std::fmt::{Display, Formatter, Result as FmtResult};

#[derive(Debug)]
pub enum ShellError {
    Io(std::io::Error),
    LineEditor(String),
    CommandNotFound { program: String },
    ExecFailed { program: String, message: String },
    Other(String),
}

impl Display for ShellError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            ShellError::Io(e) => write!(f, "{}", e),
            ShellError::LineEditor(e) => write!(f, "{}", e),
            ShellError::CommandNotFound { program } => write!(f, "command not found: {}", program),
            ShellError::ExecFailed { program, message } => write!(f, "{}: {}", program, message),
            ShellError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl From<std::io::Error> for ShellError {
    fn from(value: std::io::Error) -> Self {
        ShellError::Io(value)
    }
}

