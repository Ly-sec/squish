use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use colored::Colorize;
use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Helper};
use rustyline::history::SearchDirection;

// Cache for all available commands in PATH
static COMMAND_CACHE: OnceLock<Arc<Mutex<Option<CommandCache>>>> = OnceLock::new();

struct CommandCache {
    commands: Vec<String>,
    path_hash: u64,
}

fn hash_path() -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    if let Ok(path) = env::var("PATH") {
        path.hash(&mut hasher);
    }
    hasher.finish()
}

fn get_command_cache() -> Arc<Mutex<Option<CommandCache>>> {
    COMMAND_CACHE.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

fn build_command_cache() -> CommandCache {
    let mut commands = HashSet::new();
    
    // Collect all commands from PATH directories
    if let Ok(path_var) = env::var("PATH") {
        for dir in path_var.split(':') {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        let path = entry.path();
                        if is_executable(&path) {
                            commands.insert(name.to_string());
                        }
                    }
                }
            }
        }
    }
    
    let mut command_vec: Vec<String> = commands.into_iter().collect();
    command_vec.sort();
    
    CommandCache {
        commands: command_vec,
        path_hash: hash_path(),
    }
}

fn get_all_commands() -> Vec<String> {
    let cache = get_command_cache();
    let mut cache_guard = cache.lock().unwrap();
    
    let current_hash = hash_path();
    let needs_rebuild = cache_guard.as_ref()
        .map(|c| c.path_hash != current_hash)
        .unwrap_or(true);
    
    if needs_rebuild {
        *cache_guard = Some(build_command_cache());
    }
    
    cache_guard.as_ref()
        .map(|c| c.commands.clone())
        .unwrap_or_default()
}

#[derive(Default)]
pub struct LineHelper {
    filename: FilenameCompleter,
}

impl LineHelper {
    pub fn new() -> Self {
        Self {
            filename: FilenameCompleter::new(),
        }
    }

    fn find_commands_in_path(prefix: &str) -> Vec<Pair> {
        let all_commands = get_all_commands();
        let prefix_lower = prefix.to_lowercase();
        let mut exact_matches = Vec::new();
        let mut prefix_matches = Vec::new();
        let mut case_insensitive_matches = Vec::new();
        
        for cmd in all_commands {
            if cmd == prefix {
                // Exact match - highest priority
                exact_matches.push(Pair {
                    display: format!("{}", cmd.truecolor(200, 150, 255).bold()),
                    replacement: cmd,
                });
            } else if cmd.starts_with(prefix) {
                // Case-sensitive prefix match
                prefix_matches.push(Pair {
                    display: format!("{}", cmd.truecolor(180, 150, 255).bold()),
                    replacement: cmd,
                });
            } else if cmd.to_lowercase().starts_with(&prefix_lower) {
                // Case-insensitive prefix match - lower priority
                case_insensitive_matches.push(Pair {
                    display: format!("{}", cmd.truecolor(160, 140, 240)),
                    replacement: cmd,
                });
            }
        }
        
        // Combine: exact matches first, then case-sensitive prefix, then case-insensitive
        let mut result = exact_matches;
        result.extend(prefix_matches);
        result.extend(case_insensitive_matches);
        result
    }

    fn is_command_position(line: &str, pos: usize) -> bool {
        let before_cursor = &line[..pos];
        let trimmed = before_cursor.trim();
        
        // If it contains a slash, it's a path
        if trimmed.contains('/') {
            return false;
        }
        
        // If it's the first word (no spaces before), it's likely a command
        !trimmed.contains(' ') && !trimmed.contains('\t')
    }
}

fn is_executable(path: &Path) -> bool {
    if let Ok(metadata) = fs::metadata(path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = metadata.permissions();
            return perms.mode() & 0o111 != 0;
        }
        #[cfg(not(unix))]
        {
            // On non-Unix, check file extension
            if let Some(ext) = path.extension() {
                return matches!(ext.to_str(), Some("exe") | Some("bat") | Some("cmd"));
            }
        }
    }
    false
}

impl Helper for LineHelper {}

impl Validator for LineHelper {
    fn validate(&self, ctx: &mut rustyline::validate::ValidationContext) -> rustyline::Result<rustyline::validate::ValidationResult> {
        use rustyline::validate::ValidationResult;
        use rustyline::validate::MatchingBracketValidator;
        
        let line = ctx.input();
        
        // Check for incomplete commands (unclosed quotes, pipes, etc.)
        if Self::is_incomplete_command(line) {
            return Ok(ValidationResult::Incomplete);
        }
        
        // Use default bracket matching
        let bracket_validator = MatchingBracketValidator::new();
        bracket_validator.validate(ctx)
    }
}

impl LineHelper {
    pub fn is_incomplete_command(line: &str) -> bool {
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut backslash = false;
        
        for ch in line.chars() {
            if backslash {
                backslash = false;
                continue;
            }
            
            match ch {
                '\\' => backslash = true,
                '\'' if !in_double_quote => in_single_quote = !in_single_quote,
                '"' if !in_single_quote => in_double_quote = !in_double_quote,
                _ => {}
            }
        }
        
        // Check for trailing backslash (line continuation)
        if line.trim_end().ends_with('\\') {
            return true;
        }
        
        // Check for trailing pipe
        if line.trim_end().ends_with('|') {
            return true;
        }
        
        // Check for unclosed quotes
        in_single_quote || in_double_quote
    }
}

impl Highlighter for LineHelper {
    fn highlight_hint<'h>(&self, hint: &'h str) -> std::borrow::Cow<'h, str> {
        use std::borrow::Cow;
        Cow::Owned(hint.dimmed().to_string())
    }

    fn highlight<'l>(&self, line: &'l str, pos: usize) -> std::borrow::Cow<'l, str> {
        use std::borrow::Cow;
        
        // Only highlight when cursor is at the end of the line
        // This prevents highlighting from interfering with completion
        if pos != line.len() {
            return Cow::Borrowed(line);
        }
        
        // Syntax highlighting for commands (only when cursor is at end)
        let highlighted = Self::highlight_syntax(line);
        Cow::Owned(highlighted)
    }
}

impl LineHelper {
    fn highlight_syntax(line: &str) -> String {
        use colored::Colorize;
        
        let mut result = String::new();
        let mut chars = line.chars().peekable();
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut current_word = String::new();
        
        while let Some(ch) = chars.next() {
            match ch {
                '\'' if !in_double_quote => {
                    if in_single_quote {
                        // End of single-quoted string
                        result.push_str(&current_word.truecolor(200, 150, 255).to_string());
                        result.push('\'');
                        current_word.clear();
                        in_single_quote = false;
                    } else {
                        // Start of single-quoted string
                        if !current_word.is_empty() {
                            result.push_str(&Self::highlight_word(&current_word));
                            current_word.clear();
                        }
                        result.push('\'');
                        in_single_quote = true;
                    }
                }
                '"' if !in_single_quote => {
                    if in_double_quote {
                        // End of double-quoted string
                        result.push_str(&current_word.truecolor(200, 150, 255).to_string());
                        result.push('"');
                        current_word.clear();
                        in_double_quote = false;
                    } else {
                        // Start of double-quoted string
                        if !current_word.is_empty() {
                            result.push_str(&Self::highlight_word(&current_word));
                            current_word.clear();
                        }
                        result.push('"');
                        in_double_quote = true;
                    }
                }
                ' ' | '\t' if !in_single_quote && !in_double_quote => {
                    if !current_word.is_empty() {
                        result.push_str(&Self::highlight_word(&current_word));
                        current_word.clear();
                    }
                    result.push(ch);
                }
                '/' => {
                    current_word.push(ch);
                }
                '|' | '&' | ';' | '<' | '>' if !in_single_quote && !in_double_quote => {
                    if !current_word.is_empty() {
                        result.push_str(&Self::highlight_word(&current_word));
                        current_word.clear();
                    }
                    // Highlight operators
                    result.push_str(&ch.to_string().truecolor(255, 200, 150).bold().to_string());
                }
                _ => {
                    current_word.push(ch);
                }
            }
        }
        
        // Handle remaining word
        if !current_word.is_empty() {
            if in_single_quote || in_double_quote {
                result.push_str(&current_word.truecolor(200, 150, 255).to_string());
            } else {
                result.push_str(&Self::highlight_word(&current_word));
            }
        }
        
        result
    }
    
    fn highlight_word(word: &str) -> String {
        use colored::Colorize;
        
        // Check if it's a path
        if word.contains('/') || word.starts_with('~') || word.starts_with('.') {
            return word.truecolor(140, 180, 255).to_string();
        }
        
        // Check if it's a builtin
        let builtins = ["cd", "ll", "freqs", "help", "export", "unset", "jobs", "fg", "bg", "exit", "alias", "unalias"];
        if builtins.contains(&word) {
            return word.truecolor(200, 150, 255).bold().to_string();
        }
        
        // Check if it's a variable
        if word.starts_with('$') {
            return word.truecolor(255, 220, 150).to_string();
        }
        
        // Check if it's a number
        if word.parse::<f64>().is_ok() {
            return word.truecolor(150, 255, 180).to_string();
        }
        
        // Default: check if it looks like a command (first word)
        if word.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            // Could be a command - check if it's in PATH
            let all_commands = get_all_commands();
            if all_commands.contains(&word.to_string()) {
                return word.truecolor(180, 150, 255).bold().to_string();
            }
        }
        
        word.normal().to_string()
    }
}

impl Hinter for LineHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<Self::Hint> {
        if pos != line.len() {
            return None;
        }
        
        let trimmed = line.trim();
        
        // Don't suggest anything if the line is empty
        if trimmed.is_empty() {
            return None;
        }
        
        // Path-aware suggestions: after "cd " suggest directories
        if trimmed.starts_with("cd ") && trimmed.len() > 3 {
            let path_part = trimmed[3..].trim();
            if let Some(dir_hint) = Self::suggest_directory(path_part) {
                // Check if the suggested directory is the current directory
                if let Ok(current_dir) = env::current_dir() {
                    let full_suggestion = if path_part.is_empty() {
                        dir_hint.clone()
                    } else if path_part.starts_with("~/") {
                        if let Ok(home) = env::var("HOME") {
                            format!("{}/{}", home, &path_part[2..])
                        } else {
                            return Some(dir_hint);
                        }
                    } else if path_part == "~" {
                        env::var("HOME").ok()?
                    } else {
                        format!("{}/{}", path_part, dir_hint)
                    };
                    
                    let suggested_path = std::path::PathBuf::from(&full_suggestion);
                    if suggested_path == current_dir {
                        // Don't suggest the current directory
                        return None;
                    }
                }
                return Some(dir_hint);
            }
        }
        
        // History-based suggestions
        let history = ctx.history();
        // Search most recent first for an entry that starts with the current line
        for idx in (0..history.len()).rev() {
            if let Ok(Some(sr)) = history.get(idx, SearchDirection::Reverse) {
                let entry = &sr.entry;
                if entry.starts_with(line) && entry.len() > line.len() {
                    return Some(entry[line.len()..].to_string());
                }
            }
        }
        None
    }
}

impl LineHelper {
    fn suggest_directory(prefix: &str) -> Option<String> {
        use std::path::PathBuf;
        
        if prefix.is_empty() {
            return None;
        }
        
        let expanded = if prefix.starts_with("~/") {
            if let Ok(home) = env::var("HOME") {
                format!("{}/{}", home, &prefix[2..])
            } else {
                return None;
            }
        } else if prefix == "~" {
            env::var("HOME").ok()?
        } else {
            prefix.to_string()
        };
        
        let path = PathBuf::from(&expanded);
        
        // If path exists and is a directory, check its contents
        let (search_dir, base) = if path.is_dir() {
            (path.clone(), String::new())
        } else if let Some(parent) = path.parent() {
            if let Some(file_name) = path.file_name() {
                (parent.to_path_buf(), file_name.to_string_lossy().to_string())
            } else {
                return None;
            }
        } else {
            return None;
        };
        
        if let Ok(entries) = fs::read_dir(&search_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if base.is_empty() || name.starts_with(&base) {
                        let full_path = entry.path();
                        if full_path.is_dir() {
                            if base.is_empty() {
                                // Return the full directory name
                                return Some(format!("{}/", name));
                            } else if name.len() > base.len() {
                                // Return only the part after the prefix
                                return Some(format!("{}/", &name[base.len()..]));
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

impl Completer for LineHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        // Do not suggest anything on completely empty input
        if line[..pos].trim().is_empty() {
            return Ok((pos, Vec::new()));
        }

        // Path-only completion for `cd` arguments
        if is_cd_context(line, pos) {
            // If the argument after `cd` is empty, don't suggest anything
            let trimmed_start = line.trim_start();
            let after_cd = &trimmed_start[2..];
            if after_cd.trim().is_empty() {
                return Ok((pos, Vec::new()));
            }
            if let Some((start, pairs)) = complete_cd_only_dirs(line, pos) {
                return Ok((start, pairs));
            }
        }

        // If cursor is past the first token (i.e., there's a space before the cursor
        // after leading whitespace), use filename completion for arguments universally
        let before = &line[..pos];
        let leading_trim = before.trim_start();
        let _leading_ws = before.len() - leading_trim.len();
        let in_args = leading_trim.find(char::is_whitespace).is_some();
        if in_args {
            return self.filename.complete(line, pos, ctx);
        }

        if Self::is_command_position(line, pos) {
            // Try command completion first
            let before_cursor = &line[..pos];
            let word_start = before_cursor
                .rfind(|c: char| c.is_whitespace() || c == '|' || c == '&' || c == ';')
                .map(|i| i + 1)
                .unwrap_or(0);
            
            let prefix = &line[word_start..pos];
            
            // Builtins first (highest priority)
            let builtins = ["cd", "ll", "freqs", "help", "export", "unset", "jobs", "fg", "bg", "exit", "time"];
            let mut builtin_matches = Vec::new();
            let mut exact_builtin = None;
            
            for builtin in builtins {
                if builtin == prefix {
                    // Exact builtin match - highest priority
                    exact_builtin = Some(Pair {
                        display: format!("{}", builtin.truecolor(200, 150, 255).bold()),
                        replacement: builtin.to_string(),
                    });
                } else if builtin.starts_with(prefix) {
                    builtin_matches.push(Pair {
                        display: format!("{}", builtin.truecolor(200, 150, 255).bold()),
                        replacement: builtin.to_string(),
                    });
                }
            }
            
            // Get commands from PATH
            let path_commands = Self::find_commands_in_path(prefix);
            
            // Combine: exact builtin first, then other builtins, then PATH commands
            let mut candidates = Vec::new();
            if let Some(exact) = exact_builtin {
                candidates.push(exact);
            }
            candidates.extend(builtin_matches);
            candidates.extend(path_commands);
            
            if !candidates.is_empty() {
                // If there's only one candidate and it's an exact match, return just that
                // This helps with fish-like behavior where unique matches complete fully
                if candidates.len() == 1 && candidates[0].replacement == prefix {
                    // Already exact match, return empty to indicate completion
                    return Ok((pos, Vec::new()));
                }
                
                // If there's only one candidate after filtering exact matches, return just that
                // This makes it complete fully on next TAB
                let non_exact: Vec<_> = candidates.iter()
                    .filter(|c| c.replacement != prefix)
                    .collect();
                if non_exact.len() == 1 {
                    // Only one unique completion - return just that one so it completes fully
                    return Ok((word_start, vec![non_exact[0].clone()]));
                }
                
                return Ok((word_start, candidates));
            }
        }
        
        // Fall back to filename completion for paths
        self.filename.complete(line, pos, ctx)
    }
}

fn is_cd_context(line: &str, _pos: usize) -> bool {
    let trimmed_start = line.trim_start();
    // ensure first token is exactly "cd"
    if !trimmed_start.starts_with("cd") {
        return false;
    }
    // after cd there must be space or end
    let after_cd = &trimmed_start[2..];
    if !after_cd.is_empty() && !after_cd.starts_with(char::is_whitespace) {
        return false;
    }
    // cursor is in or after the argument area
    true
}

fn complete_cd_only_dirs(line: &str, pos: usize) -> Option<(usize, Vec<Pair>)> {
    // Find the position after "cd " - this is where the path argument starts
    let cd_pos = line.find("cd")?;
    let after_cd = &line[cd_pos + 2..];
    let after_cd_trimmed = after_cd.trim_start();
    let word_start = cd_pos + 2 + (after_cd.len() - after_cd_trimmed.len());
    
    // Get the path argument from word_start to cursor position
    let token_text = &line[word_start..pos];
    let arg_at_cursor = token_text.trim();
    let raw_prefix = if after_cd_trimmed.is_empty() { "" } else { arg_at_cursor };

    // Determine base directory and the last component prefix
    let (base_dir, base_prefix) = resolve_cd_base_and_prefix(raw_prefix)?;

    let mut scored: Vec<(u64, Pair)> = Vec::new();
    if let Ok(entries) = fs::read_dir(&base_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with(&base_prefix) {
                        // Append the missing part after the typed base_prefix
                        let display = format!("{}/", name).truecolor(140, 180, 255).to_string();
                        
                        // Build replacement: preserve what user typed up to the base_prefix, then add the directory name
                        // If base_prefix is empty, we're completing in base_dir, so use token_text as-is (with trailing / if present)
                        // If base_prefix is not empty, we need to replace the partial prefix with the full name
                        let replacement = if base_prefix.is_empty() {
                            // User typed a complete directory path, just append the new directory
                            let mut result = token_text.to_string();
                            // Ensure there's a / separator if needed
                            if !result.is_empty() && !result.ends_with('/') {
                                result.push('/');
                            }
                            result.push_str(name);
                            result.push('/');
                            result
                        } else {
                            // User typed a partial directory name, replace it
                            let keep_len = token_text.len().saturating_sub(base_prefix.len());
                            let kept = &token_text[..keep_len];
                            let separator = if kept.is_empty() || kept.ends_with('/') { "" } else { "/" };
                            format!("{}{}{}{}", kept, separator, name, "/")
                        };
                        
                        let count = crate::dirfreq::get_count(&path);
                        scored.push((count, Pair { display, replacement }));
                    }
                }
            }
        }
    }

    if scored.is_empty() {
        return None;
    }
    // Sort by frequency desc, then lexicographically by replacement
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.replacement.cmp(&b.1.replacement)));
    let pairs: Vec<Pair> = scored.into_iter().map(|(_, p)| p).collect();
    Some((word_start, pairs))
}

fn resolve_cd_base_and_prefix(input: &str) -> Option<(std::path::PathBuf, String)> {
    use std::path::PathBuf;

    if input.is_empty() {
        return Some((env::current_dir().ok()?, String::new()));
    }

    // Remove trailing slashes for processing (but we'll preserve them in the replacement)
    let normalized_input = input.trim_end_matches('/');
    if normalized_input.is_empty() {
        // Input was just "/" or "~/"
        if input.starts_with('/') {
            return Some((PathBuf::from("/"), String::new()));
        } else if input.starts_with('~') {
            return Some((env::var("HOME").ok()?.into(), String::new()));
        }
    }

    // Tilde expansion
    let expanded = if normalized_input.starts_with("~/") {
        let home = env::var("HOME").ok()?;
        format!("{}/{}", home, &normalized_input[2..])
    } else if normalized_input == "~" {
        env::var("HOME").ok()?
    } else {
        normalized_input.to_string()
    };

    let path = PathBuf::from(&expanded);
    
    // Check if it's an absolute path (starts with /) or tilde path
    if expanded.starts_with('/') || expanded.starts_with('~') {
        // Try to canonicalize if path exists, otherwise use as-is
        let normalized = path.canonicalize().ok().unwrap_or_else(|| {
            // Path doesn't exist yet, try to normalize by removing trailing components
            let mut p = path.clone();
            // Remove trailing empty components
            while p.as_os_str().to_string_lossy().ends_with('/') {
                let s = p.as_os_str().to_string_lossy();
                if s.len() <= 1 { break; }
                p.pop();
            }
            p
        });
        
        if normalized.is_dir() {
            return Some((normalized, String::new()));
        }
        // It's a partial path, find parent and prefix
        let parent = normalized.parent()?.to_path_buf();
        let base = normalized.file_name()?.to_string_lossy().to_string();
        return Some((parent, base));
    }

    // Relative path to current dir
    let cwd = env::current_dir().ok()?;
    let full_path = cwd.join(&path);
    
    // Try to canonicalize if exists
    let normalized = full_path.canonicalize().ok().unwrap_or_else(|| {
        let mut p = full_path.clone();
        while p.as_os_str().to_string_lossy().ends_with('/') {
            let s = p.as_os_str().to_string_lossy();
            if s.len() <= 1 { break; }
            p.pop();
        }
        p
    });
    
    if normalized.is_dir() {
        return Some((normalized, String::new()));
    }
    let parent = normalized.parent().map(|p| p.to_path_buf()).unwrap_or(cwd);
    let base = normalized.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    Some((parent, base))
}

