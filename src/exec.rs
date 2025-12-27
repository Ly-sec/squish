use std::env;
use std::ffi::OsStr;
use std::process::{Command, Stdio};

use crate::error::ShellError;
use crate::formatter;

pub fn run_external_command<S: AsRef<OsStr>>(program: S, args: &[String]) -> Result<i32, ShellError> {
    let program_str = program.as_ref().to_string_lossy().to_string();
    
    // Commands that should be formatted
    let should_format = matches!(program_str.as_str(), "ls" | "cat" | "cargo");
    
    let mut command = Command::new(&program);
    command.args(args);
    command.envs(env::vars());
    command.stdin(Stdio::inherit());
    
    if should_format {
        // Capture output for formatting
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        
        match command.output() {
            Ok(output) => {
                let _ = formatter::format_command_output(&program_str, args, &output);
                Ok(output.status.code().unwrap_or_default())
            }
            Err(e) => {
                use std::io::ErrorKind;
                match e.kind() {
                    ErrorKind::NotFound => Err(ShellError::CommandNotFound { program: program_str.clone() }),
                    _ => Err(ShellError::ExecFailed { program: program_str.clone(), message: e.to_string() }),
                }
            },
        }
    } else {
        // Normal execution for other commands
        command.stdout(Stdio::inherit());
        command.stderr(Stdio::inherit());
        
        match command.status() {
            Ok(status) => Ok(status.code().unwrap_or_default()),
            Err(e) => {
                use std::io::ErrorKind;
                match e.kind() {
                    ErrorKind::NotFound => Err(ShellError::CommandNotFound { program: program_str.clone() }),
                    _ => Err(ShellError::ExecFailed { program: program_str.clone(), message: e.to_string() }),
                }
            },
        }
    }
}

