use std::env;
use std::process::Command;

use colored::Colorize;
use rustyline::error::ReadlineError;
use rustyline::history::{DefaultHistory, History};
use rustyline::CompletionType;
use rustyline::config::Configurer;
use rustyline::Editor;

use crate::completion::LineHelper;
use crate::config;
use crate::error::ShellError;
use crate::shell::Shell;
use crate::shell_config::ShellConfig;

pub fn run_repl() -> Result<(), ShellError> {
    let mut rl = Editor::<LineHelper, DefaultHistory>::new().map_err(|e| ShellError::LineEditor(e.to_string()))?;
    rl.set_helper(Some(LineHelper::new()));
    
    rl.set_completion_type(CompletionType::List);
    rl.set_history_ignore_space(true);
    let _ = rl.set_history_ignore_dups(true);
    
    let mut shell = Shell::new();
    let shell_config = shell.config.clone();
    load_startup_config(&mut shell)?;

    let history_path = config::history_file();
    if let Some(path) = &history_path {
        let _ = rl.load_history(path);
    }


    let mut current_line = String::new();
    
    loop {
        let prompt_text = if current_line.is_empty() {
            generate_prompt(&shell_config, shell.last_status)
        } else {
            "  ".truecolor(200, 180, 255).dimmed().to_string() + "> "
        };
        
        match rl.readline(&prompt_text) {
            Ok(line) => {
                if current_line.is_empty() {
                    current_line = line;
                } else {
                    current_line.push('\n');
                    current_line.push_str(&line);
                }
                
                if !LineHelper::is_incomplete_command(&current_line.trim()) {
                    let full_line = current_line.trim().to_string();
                    current_line.clear();
                    
                    if !full_line.is_empty() {
                        let history = rl.history();
                        let should_add = if history.len() > 0 {
                            if let Ok(Some(last)) = history.get(history.len() - 1, rustyline::history::SearchDirection::Forward) {
                                last.entry.trim() != full_line
                            } else {
                                true
                            }
                        } else {
                            true
                        };
                        if should_add {
                            rl.add_history_entry(&full_line).ok();
                        }
                    }
                    if let Err(e) = shell.run_line(&full_line) {
                        eprintln!("squish: {}", e);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("");
                current_line.clear();
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("");
                break;
            }
            Err(e) => return Err(ShellError::LineEditor(e.to_string())),
        }
    }

    if let Some(path) = &history_path {
        let _ = rl.save_history(path);
    }

    Ok(())
}

fn load_startup_config(shell: &mut Shell) -> Result<(), ShellError> {
    let shell_config = shell.config.clone();
    for cmd in &shell_config.autostart {
        if let Err(e) = shell.run_line(cmd) {
            eprintln!("squish: autostart error: {}", e);
        }
    }
    
    if let Some(config_path) = config::config_file() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if line.contains('=') && !line.contains(' ') {
                    continue;
                }
                if line.starts_with("autostart ") || line.starts_with("autostart=") {
                    continue;
                }
                if let Err(e) = shell.run_line(line) {
                    eprintln!("squish: config error: {}", e);
                }
            }
        }
    }
    Ok(())
}


fn generate_prompt(config: &ShellConfig, last_status: i32) -> String {
    if let Some(ref format) = config.prompt_format {
        let mut result = format.clone();
        result = result.replace("%u", &env::var("USER").unwrap_or_else(|_| "user".to_string()));
        result = result.replace("%h", &hostname().unwrap_or_else(|| "host".to_string()));
        result = result.replace("%d", &current_dir_path().unwrap_or_else(|| "?".to_string()));
        result = result.replace("%s", &if last_status == 0 { "✓" } else { "✗" });
        result
    } else {
        prompt(config, last_status)
    }
}

fn apply_text_color(text: &str, color: Option<&String>) -> colored::ColoredString {
    if let Some(color_str) = color {
        apply_color(text, color_str, false)
    } else {
        text.normal()
    }
}

fn apply_bg_color(text: colored::ColoredString, color: Option<&String>) -> colored::ColoredString {
    if let Some(color_str) = color {
        if let Some((r, g, b)) = parse_rgb(color_str) {
            text.on_truecolor(r, g, b)
        } else {
            match color_str.to_lowercase().as_str() {
                "black" => text.on_black(),
                "red" => text.on_red(),
                "green" => text.on_green(),
                "yellow" => text.on_yellow(),
                "blue" => text.on_blue(),
                "magenta" => text.on_magenta(),
                "cyan" => text.on_cyan(),
                "white" => text.on_white(),
                "bright_black" | "brightblack" => text.on_bright_black(),
                "bright_red" | "brightred" => text.on_bright_red(),
                "bright_green" | "brightgreen" => text.on_bright_green(),
                "bright_yellow" | "brightyellow" => text.on_bright_yellow(),
                "bright_blue" | "brightblue" => text.on_bright_blue(),
                "bright_magenta" | "brightmagenta" => text.on_bright_magenta(),
                "bright_cyan" | "brightcyan" => text.on_bright_cyan(),
                "bright_white" | "brightwhite" => text.on_bright_white(),
                _ => text,
            }
        }
    } else {
        text
    }
}

fn apply_color(text: &str, color_str: &str, is_bg: bool) -> colored::ColoredString {
    if let Some((r, g, b)) = parse_rgb(color_str) {
        if is_bg {
            text.normal().on_truecolor(r, g, b)
        } else {
            text.truecolor(r, g, b)
        }
    } else {
        let colored = match color_str.to_lowercase().as_str() {
            "black" => text.black(),
            "red" => text.red(),
            "green" => text.green(),
            "yellow" => text.yellow(),
            "blue" => text.blue(),
            "magenta" => text.magenta(),
            "cyan" => text.cyan(),
            "white" => text.white(),
            "bright_black" | "brightblack" => text.bright_black(),
            "bright_red" | "brightred" => text.bright_red(),
            "bright_green" | "brightgreen" => text.bright_green(),
            "bright_yellow" | "brightyellow" => text.bright_yellow(),
            "bright_blue" | "brightblue" => text.bright_blue(),
            "bright_magenta" | "brightmagenta" => text.bright_magenta(),
            "bright_cyan" | "brightcyan" => text.bright_cyan(),
            "bright_white" | "brightwhite" => text.bright_white(),
            _ => text.normal(),
        };
        if is_bg {
            match color_str.to_lowercase().as_str() {
                "black" => colored.on_black(),
                "red" => colored.on_red(),
                "green" => colored.on_green(),
                "yellow" => colored.on_yellow(),
                "blue" => colored.on_blue(),
                "magenta" => colored.on_magenta(),
                "cyan" => colored.on_cyan(),
                "white" => colored.on_white(),
                "bright_black" | "brightblack" => colored.on_bright_black(),
                "bright_red" | "brightred" => colored.on_bright_red(),
                "bright_green" | "brightgreen" => colored.on_bright_green(),
                "bright_yellow" | "brightyellow" => colored.on_bright_yellow(),
                "bright_blue" | "brightblue" => colored.on_bright_blue(),
                "bright_magenta" | "brightmagenta" => colored.on_bright_magenta(),
                "bright_cyan" | "brightcyan" => colored.on_bright_cyan(),
                "bright_white" | "brightwhite" => colored.on_bright_white(),
                _ => colored,
            }
        } else {
            colored
        }
    }
}

fn parse_rgb(color_str: &str) -> Option<(u8, u8, u8)> {
    let parts: Vec<&str> = color_str.split(|c: char| c == ',' || c == ' ').collect();
    if parts.len() == 3 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            parts[0].trim().parse::<u8>(),
            parts[1].trim().parse::<u8>(),
            parts[2].trim().parse::<u8>(),
        ) {
            return Some((r, g, b));
        }
    }
    None
}

fn get_distro_icon() -> &'static str {
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("ID=") {
                let distro = line.trim_start_matches("ID=").trim_matches('"').to_lowercase();
                return match distro.as_str() {
                    "arch" | "archlinux" => "\u{f303}",
                    "ubuntu" => "\u{f31b}",
                    "debian" => "\u{e77d}",
                    "fedora" => "\u{f31a}",
                    "centos" => "\u{f304}",
                    "gentoo" => "\u{f30d}",
                    "nixos" => "\u{f313}",
                    "alpine" => "\u{f300}",
                    "manjaro" => "\u{f312}",
                    "opensuse" | "suse" => "\u{f314}",
                    "mint" | "linuxmint" => "\u{f30e}",
                    "pop" => "\u{f32a}",
                    "kali" => "\u{f327}",
                    "elementary" => "\u{f309}",
                    "void" => "\u{f32e}",
                    "raspbian" => "\u{f315}",
                    "redhat" | "rhel" => "\u{f316}",
                    "slackware" => "\u{f318}",
                    _ => "\u{f17c}",
                };
            }
        }
    }
    "\u{f17c}"
}

fn prompt(config: &ShellConfig, last_status: i32) -> String {
    let user = env::var("USER").unwrap_or_else(|_| String::from("user"));
    let host = hostname().unwrap_or_else(|| String::from("host"));
    let cwd_path = current_dir_path().unwrap_or_else(|| String::from("?"));
    let git = git_segment();
    let distro_icon = get_distro_icon();
    let sep = "\u{e0b0}";
    let top_left = "╭─".bright_black();
    let distro_text_color = config.prompt_colors.distro_text.as_ref();
    let distro_bg_color = config.prompt_colors.distro_bg.as_ref();
    let distro_text = apply_text_color(&format!(" {} ", distro_icon), distro_text_color);
    let distro_bg = if let Some(bg) = distro_bg_color {
        apply_bg_color(distro_text, Some(bg))
    } else {
        distro_text.black().on_bright_yellow()
    };
    let user_host_bg_color = config.prompt_colors.user_host_bg.as_ref();
    let default_distro_sep_color = "bright_yellow".to_string();
    let distro_sep_color = distro_bg_color.unwrap_or(&default_distro_sep_color);
    let distro_sep = apply_color(sep, distro_sep_color, false);
    let distro_sep = if let Some(_bg) = user_host_bg_color {
        apply_bg_color(distro_sep, user_host_bg_color)
    } else {
        distro_sep.on_white()
    };
    let user_host_text_color = config.prompt_colors.user_host_text.as_ref();
    let user_host_text = apply_text_color(&format!(" {}@{} ", user, host), user_host_text_color);
    let user_host_bg = if let Some(bg) = user_host_bg_color {
        apply_bg_color(user_host_text, Some(bg))
    } else {
        user_host_text.black().on_white()
    };
    let dir_bg_color = config.prompt_colors.dir_bg.as_ref();
    let default_user_sep_color = "white".to_string();
    let user_sep_color = user_host_bg_color.unwrap_or(&default_user_sep_color);
    let user_sep_colored = apply_color(sep, user_sep_color, false);
    let user_sep = if let Some(bg) = dir_bg_color {
        apply_bg_color(user_sep_colored, Some(bg))
    } else {
        user_sep_colored.on_bright_cyan()
    };
    let dir_text_color = config.prompt_colors.dir_text.as_ref();
    let dir_text = apply_text_color(&format!(" {} ", cwd_path), dir_text_color);
    let dir_bg = if let Some(bg) = dir_bg_color {
        apply_bg_color(dir_text, Some(bg))
    } else {
        dir_text.black().on_bright_cyan()
    };
    
    let mut first_line = format!("{} {}{}{}{}", 
        top_left, distro_bg, distro_sep, user_host_bg, user_sep);
    
    if let Some(g) = git {
        let git_bg_color = config.prompt_colors.git_bg.as_ref();
        let default_dir_sep_color = "bright_cyan".to_string();
        let dir_sep_color = dir_bg_color.unwrap_or(&default_dir_sep_color);
        let dir_sep_colored = apply_color(sep, dir_sep_color, false);
        let dir_sep = if let Some(bg) = git_bg_color {
            apply_bg_color(dir_sep_colored, Some(bg))
        } else {
            dir_sep_colored.on_bright_magenta()
        };
        
        let git_text_color = config.prompt_colors.git_text.as_ref();
        let git_text = apply_text_color(&format!(" {} ", g), git_text_color);
        let git_bg = if let Some(bg) = git_bg_color {
            apply_bg_color(git_text, Some(bg))
        } else {
            git_text.black().on_bright_magenta()
        };
        let git_sep = if let Some(bg) = git_bg_color {
            apply_color(sep, bg, false)
        } else {
            sep.bright_magenta()
        };
        
        first_line.push_str(&format!("{}{}{}{}", dir_bg, dir_sep, git_bg, git_sep));
    } else {
        let dir_sep = if let Some(bg) = dir_bg_color {
            apply_color(sep, bg, false)
        } else {
            sep.bright_cyan()
        };
        first_line.push_str(&format!("{}{}", dir_bg, dir_sep));
    }
    let bottom_left = "╰─".bright_black();
    let default_success_color = "bright_green".to_string();
    let default_error_color = "bright_red".to_string();
    let arrow_color = if last_status == 0 {
        config.prompt_colors.arrow_success.as_ref().unwrap_or(&default_success_color)
    } else {
        config.prompt_colors.arrow_error.as_ref().unwrap_or(&default_error_color)
    };
    let prompt_arrow = apply_color("❯", arrow_color, false);
    
    format!("{}\n{}{} ", first_line, bottom_left, prompt_arrow)
}

fn current_dir_path() -> Option<String> {
    let cwd = env::current_dir().ok()?;
    let path = cwd.to_string_lossy().to_string();
    let home = env::var("HOME").ok();
    if let Some(home_dir) = home {
        if path == home_dir {
            return Some(String::from("~"));
        }
        if path.starts_with(&home_dir) {
            let mut collapsed = String::from("~");
            collapsed.push_str(&path[home_dir.len()..]);
            return Some(collapsed);
        }
    }
    Some(path)
}

fn hostname() -> Option<String> {
    if let Ok(h) = env::var("HOSTNAME") {
        if !h.is_empty() {
            return Some(h);
        }
    }
    match std::fs::read_to_string("/proc/sys/kernel/hostname") {
        Ok(s) => Some(s.trim().to_string()),
        Err(_) => None,
    }
}

fn git_segment() -> Option<String> {
    let inside = Command::new("git")
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .output()
        .ok()?;
    if !inside.status.success() { return None; }
    let ok = String::from_utf8_lossy(&inside.stdout).trim() == "true";
    if !ok { return None; }

    let branch_out = Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .output()
        .ok();

    let branch = if let Some(out) = branch_out {
        if out.status.success() {
            Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            None
        }
    } else { None };

    let name = if let Some(b) = branch { b } else {
        let rev = Command::new("git")
            .args(["rev-parse", "--short", "HEAD"]) 
            .output()
            .ok()?;
        if !rev.status.success() { return None; }
        String::from_utf8_lossy(&rev.stdout).trim().to_string()
    };

    let status = Command::new("git")
        .args(["status", "--porcelain"]) 
        .output()
        .ok()?;
    let dirty = !String::from_utf8_lossy(&status.stdout).trim().is_empty();
    let branch_icon = "\u{e725}";
    let dirty_marker = if dirty { "*" } else { "" };
    
    Some(format!("{} {}{}", branch_icon, name, dirty_marker))
}