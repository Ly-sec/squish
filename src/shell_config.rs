use std::fs;
use std::io::BufRead;
use crate::config;

#[derive(Debug, Clone)]
pub struct PromptColors {
    pub distro_text: Option<String>,
    pub distro_bg: Option<String>,
    pub user_host_text: Option<String>,
    pub user_host_bg: Option<String>,
    pub dir_text: Option<String>,
    pub dir_bg: Option<String>,
    pub git_text: Option<String>,
    pub git_bg: Option<String>,
    pub arrow_success: Option<String>,
    pub arrow_error: Option<String>,
}

impl Default for PromptColors {
    fn default() -> Self {
        Self {
            distro_text: None,
            distro_bg: None,
            user_host_text: None,
            user_host_bg: None,
            dir_text: None,
            dir_bg: None,
            git_text: None,
            git_bg: None,
            arrow_success: None,
            arrow_error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShellConfig {
    pub prompt_format: Option<String>,
    pub show_timing: bool,
    pub timing_threshold_ms: u64,
    pub fancy_mode: bool,
    pub prompt_colors: PromptColors,
    pub autostart: Vec<String>,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            prompt_format: None,
            show_timing: true,
            timing_threshold_ms: 50, // Only show timing if command takes > 50ms
            fancy_mode: true,
            prompt_colors: PromptColors::default(),
            autostart: Vec::new(),
        }
    }
}

impl ShellConfig {
    pub fn load() -> Self {
        let mut config = Self::default();
        
        if let Some(config_file) = config::config_file() {
            if let Ok(file) = fs::File::open(&config_file) {
                let reader = std::io::BufReader::new(file);
                for line in reader.lines().flatten() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    
                    // Parse config options
                    if let Some((key, value)) = line.split_once('=') {
                        let key = key.trim();
                        let value = value.trim();
                        
                        match key {
                            "prompt" => {
                                config.prompt_format = Some(value.to_string());
                            }
                            "show_timing" => {
                                config.show_timing = value.parse().unwrap_or(true);
                            }
                            "timing_threshold_ms" => {
                                config.timing_threshold_ms = value.parse().unwrap_or(100);
                            }
                            "fancy_mode" => {
                                config.fancy_mode = value.parse().unwrap_or(true);
                            }
                            // Prompt color options
                            "prompt.distro_text" => {
                                config.prompt_colors.distro_text = Some(value.to_string());
                            }
                            "prompt.distro_bg" => {
                                config.prompt_colors.distro_bg = Some(value.to_string());
                            }
                            "prompt.user_host_text" => {
                                config.prompt_colors.user_host_text = Some(value.to_string());
                            }
                            "prompt.user_host_bg" => {
                                config.prompt_colors.user_host_bg = Some(value.to_string());
                            }
                            "prompt.dir_text" => {
                                config.prompt_colors.dir_text = Some(value.to_string());
                            }
                            "prompt.dir_bg" => {
                                config.prompt_colors.dir_bg = Some(value.to_string());
                            }
                            "prompt.git_text" => {
                                config.prompt_colors.git_text = Some(value.to_string());
                            }
                            "prompt.git_bg" => {
                                config.prompt_colors.git_bg = Some(value.to_string());
                            }
                            "prompt.arrow_success" => {
                                config.prompt_colors.arrow_success = Some(value.to_string());
                            }
                            "prompt.arrow_error" => {
                                config.prompt_colors.arrow_error = Some(value.to_string());
                            }
                            "autostart" => {
                                // Support multiple autostart commands
                                config.autostart.push(value.to_string());
                            }
                            _ => {}
                        }
                    } else if line.starts_with("autostart ") {
                        // Also support "autostart command" format
                        let cmd = line.trim_start_matches("autostart ").trim();
                        if !cmd.is_empty() {
                            config.autostart.push(cmd.to_string());
                        }
                    }
                }
            }
        }
        
        config
    }
    
}

