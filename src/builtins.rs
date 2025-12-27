use std::env;
use std::path::Path;
use std::fs;
use std::io::BufRead;
use colored::Colorize;
use chrono::{DateTime, Local};
use humansize::{format_size, DECIMAL};

use crate::error::ShellError;

pub enum BuiltinResult {
    Handled(i32),
    HandledWithOutput(i32, Vec<u8>),
    NotHandled,
}

pub fn try_handle_builtin(argv: &[String]) -> Result<BuiltinResult, ShellError> {
    if argv.is_empty() {
        return Ok(BuiltinResult::Handled(0));
    }

    match argv[0].as_str() {
        "export" => {
            if argv.len() == 1 {
                for (k, v) in env::vars() {
                    println!("{}={}", k, v);
                }
                return Ok(BuiltinResult::Handled(0));
            }
            let mut status = 0;
            for pair in &argv[1..] {
                if let Some((k, v)) = pair.split_once('=') {
                    unsafe { env::set_var(k, v) };
                } else {
                    eprintln!("export: invalid assignment: {}", pair);
                    status = 1;
                }
            }
            Ok(BuiltinResult::Handled(status))
        }
        "unset" => {
            if argv.len() < 2 { eprintln!("unset: missing name"); return Ok(BuiltinResult::Handled(1)); }
            for name in &argv[1..] { unsafe { env::remove_var(name) }; }
            Ok(BuiltinResult::Handled(0))
        }
        "cd" => {
            let target_raw = argv.get(1).cloned().unwrap_or_else(|| match env::var("HOME") {
                Ok(home) => home,
                Err(_) => String::from("/"),
            });
            let target = expand_tilde(&target_raw);
            match env::set_current_dir(&target) {
                Ok(_) => {
                    record_dir_usage(&target);
                    Ok(BuiltinResult::Handled(0))
                },
                Err(e) => {
                    eprintln!("cd: {}: {}", target, e);
                    Ok(BuiltinResult::Handled(1))
                }
            }
        }
        "ll" => {
            let target_raw = argv.get(1).cloned().unwrap_or_else(|| String::from("."));
            let target = expand_tilde(&target_raw);
            let path = Path::new(&target);
            match fancy_list_capture(path) {
                Ok((code, output)) => Ok(BuiltinResult::HandledWithOutput(code, output)),
                Err(e) => {
                    eprintln!("ll: {}: {}", target, e);
                    Ok(BuiltinResult::Handled(1))
                }
            }
        }
        "freqs" => {
            match fancy_print_dirfreq() {
                Ok(_) => Ok(BuiltinResult::Handled(0)),
                Err(e) => {
                    eprintln!("freqs: {}", e);
                    Ok(BuiltinResult::Handled(1))
                }
            }
        }
        "help" => {
            let cmd = match argv.get(1) {
                Some(s) => s,
                None => {
                    println!("Usage: help <command>\nShows a short summary and --help output if available.");
                    println!("\nBuilt-in commands:");
                    println!("  alias [name='value']  - Create or list aliases");
                    println!("  unalias <name>        - Remove an alias");
                    println!("  cd [dir]              - Change directory");
                    println!("  ll [dir]              - List directory with details");
                    println!("  freqs                - Show directory frequency stats");
                    println!("  export [var=value]    - Set environment variables");
                    println!("  unset <var>          - Unset environment variable");
                    println!("  jobs                 - List background jobs");
                    println!("  fg [job]             - Bring job to foreground");
                    println!("  bg [job]             - Resume background job");
                    println!("  time <command>       - Time command execution");
                    println!("  exit [code]          - Exit shell");
                    return Ok(BuiltinResult::Handled(0));
                }
            };
            match show_help_for(cmd) {
                Ok(code) => Ok(BuiltinResult::Handled(code)),
                Err(e) => {
                    eprintln!("help: {}", e);
                    Ok(BuiltinResult::Handled(1))
                }
            }
        }
        "jobs" => {
            Ok(BuiltinResult::NotHandled)
        }
        "fg" | "bg" => {
            Ok(BuiltinResult::NotHandled)
        }
        "exit" => {
            let code = argv.get(1).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
            std::process::exit(code);
        }
        _ => Ok(BuiltinResult::NotHandled),
    }
}

#[inline]
fn record_dir_usage(path: &str) {
    if path.is_empty() { return; }
    #[allow(unused_imports)]
    use crate::dirfreq::increment_dir_usage;
    let p = Path::new(path);
    increment_dir_usage(p);
}

fn expand_tilde(input: &str) -> String {
    if let Ok(home) = env::var("HOME") {
        if input == "~" {
            return home;
        }
        if let Some(rest) = input.strip_prefix("~/") {
            let mut s = home;
            s.push('/');
            s.push_str(rest);
            return s;
        }
    }
    input.to_string()
}

fn fancy_list_capture(dir: &Path) -> Result<(i32, Vec<u8>), std::io::Error> {
    use std::io::Write;
    let mut output = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(dir)?.flatten().collect();
    entries.sort_by_key(|e| e.file_name().to_ascii_lowercase());
    entries.sort_by_key(|e| match e.file_type() { Ok(t) if t.is_dir() => 0, _ => 1 });

    let header = format!("{:2}  {:>8}  {:<19}  {}", "T", "Size", "Modified", "Name");
    writeln!(output, "{}", header.bold().underline())?;

    for entry in entries {
        let path = entry.path();
        let md = match entry.metadata() { Ok(m) => m, Err(_) => continue };
        let file_type = if md.is_dir() { 'd' } else if md.is_symlink() { 'l' } else { '-' };
        let size = if md.is_dir() { String::from("—") } else { format_size(md.len(), DECIMAL) };
        let modified = md.modified().ok()
            .and_then(|t| DateTime::<Local>::from(t).format("%Y-%m-%d %H:%M").to_string().into())
            .unwrap_or_else(|| String::from("—"));
        let name = entry.file_name().to_string_lossy().to_string();
        let colored_name = colorize_name(&path, &name, &md);

        writeln!(output,
            "{}  {:>8}  {:<19}  {}",
            style_type(file_type),
            size.dimmed(),
            modified.dimmed(),
            colored_name
        )?;
    }
    Ok((0, output))
}

fn style_type(t: char) -> colored::ColoredString {
    match t {
        'd' => "d".truecolor(140, 180, 255),
        'l' => "l".truecolor(200, 150, 255),
        _ => "-".dimmed(),
    }
}

fn colorize_name(path: &Path, name: &str, md: &fs::Metadata) -> colored::ColoredString {
    if md.is_dir() {
        name.truecolor(140, 180, 255).bold()
    } else if md.is_symlink() {
        name.truecolor(200, 150, 255)
    } else {
        match path.extension().and_then(|e| e.to_str()) {
            Some("rs") => name.truecolor(255, 150, 180),
            Some("md") => name.truecolor(240, 160, 255),
            Some("toml") => name.truecolor(255, 220, 150),
            Some("png") | Some("jpg") | Some("jpeg") | Some("gif") => name.truecolor(150, 255, 180),
            Some("zip") | Some("tar") | Some("gz") => name.truecolor(255, 220, 150),
            Some("sh") => name.truecolor(150, 255, 180),
            _ => name.normal(),
        }
    }
}

fn collapse_home(path: &str) -> String {
    if let Ok(home) = env::var("HOME") {
        if path == home { return String::from("~"); }
        if let Some(rem) = path.strip_prefix(&home) {
            let mut s = String::from("~");
            s.push_str(rem);
            return s;
        }
    }
    path.to_string()
}

fn fancy_print_dirfreq() -> Result<(), std::io::Error> {
    use crate::config;
    let Some(file) = config::dirfreq_file() else { return Ok(()); };
    let f = match fs::File::open(&file) { Ok(f) => f, Err(_) => return Ok(()) };
    let rd = std::io::BufReader::new(f);
    let mut rows: Vec<(u64, String)> = Vec::new();
    for line in rd.lines().flatten() {
        if let Some((p, c)) = line.rsplit_once('\t') {
            if let Ok(n) = c.parse::<u64>() {
                rows.push((n, collapse_home(p)));
            }
        }
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

    let header = format!("{:>8}  {}", "Count", "Directory");
    println!("{}", header.bold().underline());
    for (n, p) in rows {
        println!("{:>8}  {}", n.to_string().truecolor(150, 255, 180), p.truecolor(140, 180, 255));
    }
    Ok(())
}

fn show_help_for(cmd: &str) -> Result<i32, std::io::Error> {
    use std::process::Command;
    if which::which("whatis").is_ok() {
        if let Ok(out) = Command::new("whatis").arg(cmd).output() {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                if !text.trim().is_empty() { println!("{}", text.trim()); }
            }
        }
    }
    if let Ok(out) = Command::new(cmd).arg("--help").output() {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            println!("{}", text);
            return Ok(0);
        }
    }
    if let Ok(out) = Command::new(cmd).arg("-h").output() {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            println!("{}", text);
            return Ok(0);
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::NotFound, format!("no help available for {}", cmd)))
}

