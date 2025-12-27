use std::io::{self, Write};
use std::path::Path;
use std::process::Output;
use colored::Colorize;
use chrono::{DateTime, Local};
use humansize::{format_size, DECIMAL};

pub fn format_command_output(program: &str, args: &[String], output: &Output) -> io::Result<()> {
    match program {
        "ls" => format_ls_output(&output),
        "cat" => format_cat_output(args, &output),
        "cargo" => format_cargo_output(args, &output),
        _ => format_generic_output(&output),
    }
}

fn format_ls_output(output: &Output) -> io::Result<()> {
    if !output.status.success() {
        io::stderr().write_all(&output.stderr)?;
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    
    if lines.is_empty() {
        return Ok(());
    }

    // Parse ls output - try to detect format
    // Simple format: just filenames
    // Long format: permissions, links, user, group, size, date, name
    
    let mut entries = Vec::new();
    
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        
        // Check if it's long format (starts with permissions like "drwxr-xr-x")
        if line.starts_with('-') || line.starts_with('d') || line.starts_with('l') {
            if let Some(parsed) = parse_ls_long_line(line) {
                entries.push(parsed);
            } else {
                // Fallback: treat as simple filename
                entries.push(FileEntry {
                    name: line.to_string(),
                    is_dir: false,
                    is_symlink: false,
                    size: None,
                    modified: None,
                });
            }
        } else {
            // Simple format - just filename
            entries.push(FileEntry {
                name: line.to_string(),
                is_dir: false,
                is_symlink: false,
                size: None,
                modified: None,
            });
        }
    }

    // Try to get actual file metadata for better formatting
    let cwd = std::env::current_dir().unwrap_or_default();
    for entry in &mut entries {
        let path = cwd.join(&entry.name);
        if let Ok(metadata) = std::fs::metadata(&path) {
            entry.is_dir = metadata.is_dir();
            entry.is_symlink = metadata.is_symlink();
            if entry.size.is_none() {
                entry.size = Some(metadata.len());
            }
            if entry.modified.is_none() {
                entry.modified = metadata.modified().ok();
            }
        }
    }

    print_fancy_list(&entries);
    Ok(())
}

struct FileEntry {
    name: String,
    is_dir: bool,
    is_symlink: bool,
    size: Option<u64>,
    modified: Option<std::time::SystemTime>,
}

fn parse_ls_long_line(line: &str) -> Option<FileEntry> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 9 {
        return None;
    }
    
    let perms = parts[0];
    let is_dir = perms.starts_with('d');
    let is_symlink = perms.starts_with('l');
    
    let size_str = parts.get(4)?;
    let size = size_str.parse::<u64>().ok();
    
    // Date is usually parts[5], [6], [7] or [6], [7], [8]
    // Name is the last part (or last few if it contains spaces)
    let name = parts[8..].join(" ");
    
    Some(FileEntry {
        name,
        is_dir,
        is_symlink,
        size,
        modified: None, // Would need to parse date
    })
}

fn print_fancy_list(entries: &[FileEntry]) {
    if entries.is_empty() {
        return;
    }

    // Calculate column widths
    let max_size_len = entries.iter()
        .map(|e| {
            if e.is_dir {
                2
            } else if let Some(s) = e.size {
                format_size(s, DECIMAL).len()
            } else {
                2
            }
        })
        .max()
        .unwrap_or(8)
        .max(8);
    
    let max_name_len = entries.iter()
        .map(|e| e.name.len())
        .max()
        .unwrap_or(20);
    
    // Top border
    println!("┌{}┬{}┬{}┬{}┐", 
        "─".repeat(2),
        "─".repeat(max_size_len.max(8)),
        "─".repeat(19),
        "─".repeat(max_name_len.max(20))
    );
    
    // Header (ANSI-aware padding)
    let t_hdr = "T".bold().to_string();
    let size_hdr = "Size".bold().to_string();
    let mod_hdr = "Modified".bold().to_string();
    let name_hdr = "Name".bold().to_string();
    let w_size = max_size_len.max(8);
    let w_name = max_name_len.max(20);
    let t_pad = 2usize.saturating_sub(visible_width(&t_hdr));
    let size_pad = w_size.saturating_sub(visible_width(&size_hdr));
    let mod_pad = 19usize.saturating_sub(visible_width(&mod_hdr));
    let name_pad = w_name.saturating_sub(visible_width(&name_hdr));

    print!("│{}{}│", t_hdr, " ".repeat(t_pad));
    print!("{}{}│", size_hdr, " ".repeat(size_pad));
    print!("{}{}│", mod_hdr, " ".repeat(mod_pad));
    println!("{}{}│", name_hdr, " ".repeat(name_pad));
    
    // Separator
    println!("├{}┼{}┼{}┼{}┤",
        "─".repeat(2),
        "─".repeat(max_size_len.max(8)),
        "─".repeat(19),
        "─".repeat(max_name_len.max(20))
    );

    for entry in entries {
        let file_type = if entry.is_dir {
            "d".truecolor(140, 180, 255).bold()
        } else if entry.is_symlink {
            "l".truecolor(200, 150, 255).bold()
        } else {
            "-".dimmed()
        };

        // Plain strings for width calculation
        let size_plain = if entry.is_dir {
            "-".to_string()
        } else if let Some(s) = entry.size { format_size(s, DECIMAL) } else { "-".to_string() };
        let modified_plain = entry
            .modified
            .and_then(|t| DateTime::<Local>::from(t).format("%Y-%m-%d %H:%M").to_string().into())
            .unwrap_or_else(|| String::from("-"));
        let name_plain = &entry.name;

        let w_size = max_size_len.max(8);
        let w_name = max_name_len.max(20);

        let size_pad = w_size.saturating_sub(size_plain.len());
        let name_pad = w_name.saturating_sub(name_plain.len());
        let mod_pad = 19usize.saturating_sub(modified_plain.len());

        let colored_name = colorize_name(name_plain, entry.is_dir, entry.is_symlink);

        // Print with manual padding so ANSI codes don't break widths
        let t_pad = 2usize.saturating_sub(visible_width(&file_type.to_string()));
        print!("│{}{}│", file_type, " ".repeat(t_pad));
        print!("{}{}│", size_plain.dimmed(), " ".repeat(size_pad));
        print!("{}{}│", modified_plain.dimmed(), " ".repeat(mod_pad));
        println!("{}{}│", colored_name, " ".repeat(name_pad));
    }
    
    // Bottom border
    println!("└{}┴{}┴{}┴{}┘",
        "─".repeat(2),
        "─".repeat(max_size_len.max(8)),
        "─".repeat(19),
        "─".repeat(max_name_len.max(20))
    );
}

fn colorize_name(name: &str, is_dir: bool, is_symlink: bool) -> colored::ColoredString {
    if is_dir {
        name.truecolor(140, 180, 255).bold()
    } else if is_symlink {
        name.truecolor(200, 150, 255)
    } else {
        let path = Path::new(name);
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

// Utilities to handle ANSI-colored widths
fn strip_ansi_codes(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' { // ESC
            if let Some('[') = chars.peek().copied() {
                chars.next();
                // skip until a letter (usually 'm')
                while let Some(c) = chars.next() {
                    if ('a'..='z').contains(&c) || ('A'..='Z').contains(&c) {
                        break;
                    }
                }
                continue;
            }
        }
        out.push(ch);
    }
    out
}

fn visible_width(s: &str) -> usize {
    strip_ansi_codes(s).chars().count()
}

fn truncate_visual(s: &str, width: usize) -> String {
    let mut out = String::new();
    let mut acc = 0usize;
    let mut iter = s.chars().peekable();
    let mut in_esc = false;
    while let Some(ch) = iter.next() {
        if in_esc {
            out.push(ch);
            if ('a'..='z').contains(&ch) || ('A'..='Z').contains(&ch) { in_esc = false; }
            continue;
        }
        if ch == '\u{1b}' {
            in_esc = true;
            out.push(ch);
            continue;
        }
        if acc < width {
            out.push(ch);
            acc += 1;
        } else {
            break;
        }
    }
    out
}

fn format_cat_output(args: &[String], output: &Output) -> io::Result<()> {
    if !output.status.success() {
        io::stderr().write_all(&output.stderr)?;
        return Ok(());
    }

    // Try to detect file type from first argument
    let file_path = args.first().map(|s| Path::new(s));
    let ext = file_path.and_then(|p| p.extension()).and_then(|e| e.to_str());
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Simple syntax highlighting based on extension
    match ext {
        Some("rs") => print_rust_like(&stdout),
        Some("toml") => print_toml_like(&stdout),
        Some("json") => print_json_like(&stdout),
        Some("sh") | Some("bash") => print_shell_like(&stdout),
        _ => {
            // Default: just print with line numbers
            print_with_line_numbers(&stdout);
        }
    }
    
    Ok(())
}

fn print_rust_like(content: &str) {
    let lines: Vec<&str> = content.lines().collect();
    let max_line_num = lines.len();
    let num_width = max_line_num.to_string().len().max(4);
    
    // Top border
    println!("┌{}┬{}┐", "─".repeat(num_width), "─".repeat(80));
    
    for (i, line) in lines.iter().enumerate() {
        let num = format!("{:width$}", i + 1, width = num_width);
        let highlighted = highlight_rust_line(line);
        
        // Truncate long lines for display
        let display_line = truncate_visual(&highlighted, 80);
        let pad = 80usize.saturating_sub(visible_width(&display_line));
        println!("│{}│{}{}│", num.bright_black().bold(), display_line, " ".repeat(pad));
    }
    
    // Bottom border
    println!("└{}┴{}┘", "─".repeat(num_width), "─".repeat(80));
}

fn highlight_rust_line(line: &str) -> String {
    // Enhanced highlighting with better parsing
    let mut result = String::new();
    let mut chars = line.chars().peekable();
    let mut in_string = false;
    let mut in_comment = false;
    let mut current_word = String::new();
    
    while let Some(ch) = chars.next() {
        if in_comment {
            result.push_str(&ch.to_string().bright_black());
            continue;
        }
        
        if ch == '/' && chars.peek() == Some(&'/') {
            in_comment = true;
            result.push_str(&"//".bright_black().to_string());
            chars.next(); // consume second /
            continue;
        }
        
        if ch == '"' && !in_string {
            in_string = true;
            result.push('"');
            continue;
        } else if ch == '"' && in_string {
            in_string = false;
            result.push_str(&format!("{}{}", current_word.truecolor(150, 255, 180), "\"".truecolor(150, 255, 180)));
            current_word.clear();
            continue;
        }
        
        if in_string {
            current_word.push(ch);
            continue;
        }
        
        if ch.is_alphanumeric() || ch == '_' {
            current_word.push(ch);
        } else {
            if !current_word.is_empty() {
                let colored = colorize_rust_token(&current_word);
                result.push_str(&colored);
                current_word.clear();
            }
            
            if ch.is_whitespace() {
                result.push(ch);
            } else {
                result.push_str(&ch.to_string().dimmed());
            }
        }
    }
    
        if !current_word.is_empty() {
            if in_string {
                result.push_str(&current_word.truecolor(150, 255, 180).to_string());
            } else {
                result.push_str(&colorize_rust_token(&current_word));
            }
        }
    
    result
}

fn colorize_rust_token(token: &str) -> String {
    let keywords = ["fn", "let", "mut", "pub", "use", "mod", "struct", "enum", "impl", 
                     "trait", "if", "else", "match", "return", "Ok", "Err", "Some", "None",
                     "true", "false", "self", "Self", "async", "await", "const", "static"];
    
    if keywords.contains(&token) {
        token.truecolor(140, 180, 255).bold().to_string()
    } else if token.parse::<i64>().is_ok() || token.parse::<f64>().is_ok() {
        token.truecolor(255, 220, 150).to_string()
    } else if token.starts_with('&') {
        format!("&{}", token[1..].truecolor(200, 150, 255))
    } else {
        token.to_string()
    }
}

fn print_toml_like(content: &str) {
    let lines: Vec<&str> = content.lines().collect();
    let max_line_num = lines.len();
    let num_width = max_line_num.to_string().len().max(4);
    
    println!("┌{}┬{}┐", "─".repeat(num_width), "─".repeat(80));
    
    for (i, line) in lines.iter().enumerate() {
        let num = format!("{:width$}", i + 1, width = num_width);
        let colored = if line.trim_start().starts_with('[') {
            line.truecolor(200, 150, 255).bold().to_string()
        } else if line.trim_start().starts_with('#') {
            line.bright_black().to_string()
        } else if line.contains('=') {
            let parts: Vec<&str> = line.splitn(2, '=').collect();
            if parts.len() == 2 {
                format!("{}{}{}", 
                    parts[0].trim().truecolor(255, 220, 150).bold(),
                    " = ".dimmed(),
                    parts[1].trim().truecolor(150, 255, 180)
                )
            } else {
                line.to_string()
            }
        } else {
            line.to_string()
        };
        
        let display_line = truncate_visual(&colored, 80);
        let pad = 80usize.saturating_sub(visible_width(&display_line));
        println!("│{}│{}{}│", num.bright_black().bold(), display_line, " ".repeat(pad));
    }
    
    println!("└{}┴{}┘", "─".repeat(num_width), "─".repeat(80));
}

fn print_json_like(content: &str) {
    for (i, line) in content.lines().enumerate() {
        let num = format!("{:4}", i + 1);
        println!("{} {}", num.dimmed(), line);
    }
}

fn print_shell_like(content: &str) {
    for (i, line) in content.lines().enumerate() {
        let num = format!("{:4}", i + 1);
        let colored = if line.trim_start().starts_with('#') {
            line.bright_black().to_string()
        } else {
            line.to_string()
        };
        println!("{} {}", num.dimmed(), colored);
    }
}

fn print_with_line_numbers(content: &str) {
    let lines: Vec<&str> = content.lines().collect();
    let max_line_num = lines.len();
    let num_width = max_line_num.to_string().len().max(4);
    
    println!("┌{}┬{}┐", "─".repeat(num_width), "─".repeat(80));
    
    for (i, line) in lines.iter().enumerate() {
        let num = format!("{:width$}", i + 1, width = num_width);
        let display_line = truncate_visual(line, 80);
        let pad = 80usize.saturating_sub(visible_width(&display_line));
        println!("│{}│{}{}│", num.bright_black().bold(), display_line, " ".repeat(pad));
    }
    
    println!("└{}┴{}┘", "─".repeat(num_width), "─".repeat(80));
}

fn format_generic_output(output: &Output) -> io::Result<()> {
    io::stdout().write_all(&output.stdout)?;
    io::stderr().write_all(&output.stderr)?;
    Ok(())
}

fn format_cargo_output(_args: &[String], output: &Output) -> io::Result<()> {
    // Colorize cargo/rustc diagnostics: warnings yellow, errors red, help cyan, code lines dimmed
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    for line in stdout.lines() {
        println!("{}", colorize_diagnostic_line(line));
    }
    for line in stderr.lines() {
        eprintln!("{}", colorize_diagnostic_line(line));
    }
    Ok(())
}

fn colorize_diagnostic_line(line: &str) -> String {
    let trimmed = line.trim_start();
    if trimmed.starts_with("error[") || trimmed.starts_with("error:") {
        line.truecolor(255, 120, 180).bold().to_string()
    } else if trimmed.starts_with("warning[") || trimmed.starts_with("warning:") {
        line.truecolor(255, 220, 150).bold().to_string()
    } else if trimmed.starts_with("help:") || trimmed.starts_with("note:") {
        line.truecolor(180, 160, 255).to_string()
    } else if trimmed.starts_with("--> ") || trimmed.starts_with("| ") || trimmed.starts_with("  = ") {
        line.dimmed().to_string()
    } else {
        line.to_string()
    }
}

