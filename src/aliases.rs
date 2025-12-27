use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufWriter, Write};
use std::path::PathBuf;
use crate::config;

pub struct AliasManager {
    aliases: HashMap<String, String>,
    config_path: Option<PathBuf>,
}

impl AliasManager {
    pub fn new() -> Self {
        let config_path = config::alias_file();
        let mut manager = Self {
            aliases: HashMap::new(),
            config_path: config_path.clone(),
        };
        if let Some(path) = &config_path {
            let _ = manager.load_from_file(path);
        }
        manager
    }

    pub fn set(&mut self, name: String, value: String) {
        self.aliases.insert(name, value);
        if let Some(path) = &self.config_path {
            let _ = self.save_to_file(path);
        }
    }

    pub fn unset(&mut self, name: &str) -> bool {
        let removed = self.aliases.remove(name).is_some();
        if removed {
            if let Some(path) = &self.config_path {
                let _ = self.save_to_file(path);
            }
        }
        removed
    }

    pub fn get(&self, name: &str) -> Option<&String> {
        self.aliases.get(name)
    }

    pub fn list(&self) -> &HashMap<String, String> {
        &self.aliases
    }

    pub fn expand(&self, line: &str) -> String {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return line.to_string();
        }

        // Check if first word is an alias
        if let Some(alias_value) = self.get(parts[0]) {
            let mut result = alias_value.clone();
            // Append remaining arguments
            if parts.len() > 1 {
                result.push(' ');
                result.push_str(&parts[1..].join(" "));
            }
            result
        } else {
            line.to_string()
        }
    }

    fn load_from_file(&mut self, path: &PathBuf) -> std::io::Result<()> {
        let file = fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        
        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            
            if let Some(rest) = trimmed.strip_prefix("alias ") {
                if let Some((name, value)) = Self::parse_alias_line(rest) {
                    self.aliases.insert(name, value);
                }
            }
        }
        Ok(())
    }

    fn save_to_file(&self, path: &PathBuf) -> std::io::Result<()> {
        let file = fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        
        writeln!(writer, "# Squish aliases - auto-generated")?;
        writeln!(writer, "# Format: alias name='value'")?;
        writeln!(writer, "")?;
        
        let mut sorted: Vec<_> = self.aliases.iter().collect();
        sorted.sort_by_key(|(k, _)| *k);
        
        for (name, value) in sorted {
            let escaped = if value.contains(' ') || value.contains('\'') || value.contains('"') {
                format!("'{}'", value.replace('\'', "'\\''"))
            } else {
                value.clone()
            };
            writeln!(writer, "alias {}={}", name, escaped)?;
        }
        Ok(())
    }

    fn parse_alias_line(line: &str) -> Option<(String, String)> {
        let mut chars = line.chars().peekable();
        let mut name = String::new();
        
        while let Some(&c) = chars.peek() {
            if c == '=' {
                chars.next();
                break;
            }
            name.push(c);
            chars.next();
        }
        
        if name.is_empty() {
            return None;
        }
        
        let name = name.trim().to_string();
        
        let mut value = String::new();
        let quote_char = chars.peek().copied();
        
        if quote_char == Some('\'') || quote_char == Some('"') {
            let quote = quote_char.unwrap();
            chars.next();
            while let Some(c) = chars.next() {
                if c == quote {
                    break;
                }
                value.push(c);
            }
        } else {
            while let Some(c) = chars.next() {
                value.push(c);
            }
        }
        
        Some((name, value.trim().to_string()))
    }
}

impl Default for AliasManager {
    fn default() -> Self {
        Self::new()
    }
}


