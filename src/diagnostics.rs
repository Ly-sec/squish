use colored::Colorize;

use crate::error::ShellError;

pub fn print_error(err: &ShellError) {
    match err {
        ShellError::CommandNotFound { program } => {
            eprintln!("{} {}", "error:".truecolor(255, 120, 180).bold(), format!("command not found: {}", program).truecolor(255, 150, 200));
            let suggestions = top_suggestions(program, 3);
            if !suggestions.is_empty() {
                let list = suggestions.join(", ");
                eprintln!("{} {} {}", "help:".truecolor(180, 160, 255), "did you mean".truecolor(180, 160, 255), list.truecolor(200, 150, 255).bold());
            }
            if let Some(hint) = install_hint(program) {
                eprintln!("{} {}", "help:".truecolor(180, 160, 255), hint.truecolor(180, 160, 255));
            }
            if let Some(path_note) = truncated_path_note() {
                eprintln!("{} {}", "note:".bright_black(), path_note.bright_black());
            }
        }
        ShellError::ExecFailed { program, message } => {
            eprintln!("{} {}", "error:".truecolor(255, 120, 180).bold(), format!("{}: {}", program, message).truecolor(255, 150, 200));
        }
        ShellError::Io(e) => {
            eprintln!("{} {}", "error:".truecolor(255, 120, 180).bold(), e.to_string().truecolor(255, 150, 200));
        }
        ShellError::LineEditor(e) => {
            eprintln!("{} {}", "error:".truecolor(255, 120, 180).bold(), e.to_string().truecolor(255, 150, 200));
        }
        ShellError::Other(msg) => {
            eprintln!("{} {}", "error:".truecolor(255, 120, 180).bold(), msg.truecolor(255, 150, 200));
        }
    }
}

fn top_suggestions(input: &str, max_n: usize) -> Vec<String> {
    let mut candidates: Vec<String> = builtins()
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(':') {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        candidates.push(name.to_string());
                    }
                }
            }
        }
    }
    candidates.sort();
    candidates.dedup();

    let mut scored: Vec<(usize, String)> = candidates
        .into_iter()
        .map(|c| (edit_distance(input, &c), c))
        .filter(|(d, _)| *d <= 2)
        .collect();
    scored.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    scored.into_iter().take(max_n).map(|(_, s)| s).collect()
}

fn builtins() -> &'static [&'static str] { &["cd", "ll", "freqs", "help", "export", "unset", "jobs", "fg", "bg", "exit"] }

fn edit_distance(a: &str, b: &str) -> usize {
    let mut dp = vec![vec![0; b.len() + 1]; a.len() + 1];
    for i in 0..=a.len() { dp[i][0] = i; }
    for j in 0..=b.len() { dp[0][j] = j; }
    for (i, ca) in a.chars().enumerate() {
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            dp[i + 1][j + 1] = (dp[i][j + 1] + 1).min(dp[i + 1][j] + 1).min(dp[i][j] + cost);
        }
    }
    dp[a.len()][b.len()]
}

fn install_hint(cmd: &str) -> Option<String> {
    // Detect common package managers
    let mgrs = detect_pkg_mgrs();
    if mgrs.is_empty() { return None; }
    let cmd_str = cmd.to_string();
    let mut hints = Vec::new();
    for m in mgrs {
        match m.as_str() {
            "pacman" => hints.push(format!("try: sudo pacman -S {}", cmd_str)),
            "apt" => hints.push(format!("try: sudo apt install {}", cmd_str)),
            "dnf" => hints.push(format!("try: sudo dnf install {}", cmd_str)),
            "zypper" => hints.push(format!("try: sudo zypper install {}", cmd_str)),
            "brew" => hints.push(format!("try: brew install {}", cmd_str)),
            _ => (),
        }
    }
    if hints.is_empty() { None } else { Some(hints.join("  |  ")) }
}

fn detect_pkg_mgrs() -> Vec<String> {
    let candidates = ["pacman", "apt", "dnf", "zypper", "brew"];
    let mut found = Vec::new();
    for c in candidates.iter() {
        if which::which(c).is_ok() { found.push(c.to_string()); }
    }
    found
}

fn truncated_path_note() -> Option<String> {
    let path = std::env::var("PATH").ok()?;
    let parts: Vec<&str> = path.split(':').collect();
    if parts.is_empty() { return None; }
    let shown = parts.iter().take(3).cloned().collect::<Vec<_>>().join(":");
    let rest = parts.len().saturating_sub(3);
    if rest > 0 {
        Some(format!("searched PATH: {} (+{} more)", shown, rest))
    } else {
        Some(format!("searched PATH: {}", shown))
    }
}


