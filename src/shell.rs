use crate::builtins::{try_handle_builtin, BuiltinResult};
use crate::error::ShellError;
use crate::exec::run_external_command;
use crate::diagnostics;
use crate::parser::{parse_command_line, CommandPart};
use crate::jobs::JobManager;
use crate::aliases::AliasManager;
use crate::shell_config::ShellConfig;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::Instant;

#[derive(Debug, Clone)]
struct TimingInfo {
    real: f64,
    user: f64,
    system: f64,
}

pub struct Shell {
    pub last_status: i32,
    pub jobs: JobManager,
    pub aliases: AliasManager,
    pub config: ShellConfig,
    pub last_command_time: Option<f64>,
}

impl Shell {
    pub fn new() -> Self {
        Self { 
            last_status: 0,
            jobs: JobManager::new(),
            aliases: AliasManager::new(),
            config: ShellConfig::load(),
            last_command_time: None,
        }
    }

    pub fn run_line(&mut self, line: &str) -> Result<(), ShellError> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return Ok(());
        }

        let expanded = self.aliases.expand(trimmed);
        let start = Instant::now();

        let result = match parse_command_line(&expanded) {
            Ok(cmd) => {
                self.last_status = self.execute_command(&cmd)?;
                Ok(())
            }
            Err(e) => {
                diagnostics::print_error(&e);
                self.last_status = 1;
                Ok(())
            }
        };

        let elapsed = start.elapsed();
        let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
        self.last_command_time = Some(elapsed_ms);

        if self.config.show_timing && (self.config.timing_threshold_ms == 0 || elapsed_ms >= self.config.timing_threshold_ms as f64) {
            self.display_timing(elapsed_ms);
        }

        result
    }

    fn display_timing(&self, elapsed_ms: f64) {
        use colored::Colorize;
        
        let time_str = if elapsed_ms < 1000.0 {
            format!("{:.0}ms", elapsed_ms)
        } else {
            format!("{:.2}s", elapsed_ms / 1000.0)
        };
        
        let color = if elapsed_ms > 5000.0 {
            time_str.truecolor(255, 120, 120).bold()
        } else if elapsed_ms > 1000.0 {
            time_str.truecolor(255, 200, 120)
        } else {
            time_str.truecolor(150, 255, 180)
        };
        eprintln!("⏱ {}", color);
    }

    fn execute_command(&mut self, cmd: &CommandPart) -> Result<i32, ShellError> {
        match cmd {
            CommandPart::Simple { argv, background } => self.execute_simple(argv, *background),
            CommandPart::Pipe { left, right } => self.execute_pipe(left, right),
            CommandPart::RedirectOut { cmd, file, append } => self.execute_redirect_out(cmd, file, *append),
            CommandPart::RedirectIn { cmd, file } => self.execute_redirect_in(cmd, file),
            CommandPart::Chain { left, right, and } => self.execute_chain(left, right, *and),
        }
    }

    fn execute_simple(&mut self, argv: &[String], background: bool) -> Result<i32, ShellError> {
        if argv.is_empty() {
            return Ok(0);
        }

        if argv[0] == "time" {
            if argv.len() < 2 {
                eprintln!("time: missing command");
                return Ok(1);
            }
            let cmd_argv = &argv[1..];
            let (status, timing) = self.execute_with_timing(cmd_argv, background)?;
            self.display_detailed_timing(&timing);
            return Ok(status);
        }

        match argv[0].as_str() {
            "alias" => {
                if argv.len() == 1 {
                    for (name, value) in self.aliases.list() {
                        println!("alias {}='{}'", name, value);
                    }
                    return Ok(0);
                }
                let alias_def = argv[1..].join(" ");
                if let Some((name, value)) = alias_def.split_once('=') {
                    let value = value.trim();
                    let value = if (value.starts_with('\'') && value.ends_with('\'')) ||
                                   (value.starts_with('"') && value.ends_with('"')) {
                        &value[1..value.len()-1]
                    } else {
                        value
                    };
                    self.aliases.set(name.trim().to_string(), value.to_string());
                    return Ok(0);
                } else {
                    eprintln!("alias: invalid format: {}", alias_def);
                    return Ok(1);
                }
            }
            "unalias" => {
                if argv.len() < 2 {
                    eprintln!("unalias: missing alias name");
                    return Ok(1);
                }
                let mut status = 0;
                for name in &argv[1..] {
                    if !self.aliases.unset(name) {
                        eprintln!("unalias: {}: not found", name);
                        status = 1;
                    }
                }
                return Ok(status);
            }
            _ => {}
        }

        match argv[0].as_str() {
            "jobs" => {
                self.jobs.remove_finished();
                for job in self.jobs.list_jobs() {
                    let status = if let Ok(child_opt) = job.child.lock() {
                        if child_opt.is_some() { "Running" } else { "Done" }
                    } else {
                        "Unknown"
                    };
                    println!("[{}] {} {}", job.id, status, job.command);
                }
                return Ok(0);
            }
            "fg" => {
                let id = argv.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);
                if let Some(job) = self.jobs.get_job(id) {
                    if let Ok(mut child_opt) = job.child.lock() {
                        if let Some(mut child) = child_opt.take() {
                            let status = child.wait()?;
                            return Ok(status.code().unwrap_or(1));
                        }
                    }
                }
                eprintln!("fg: job {} not found", id);
                return Ok(1);
            }
            "bg" => {
                return Ok(0);
            }
            _ => {}
        }

        match try_handle_builtin(argv)? {
            BuiltinResult::Handled(status) => Ok(status),
            BuiltinResult::HandledWithOutput(status, _) => Ok(status),
            BuiltinResult::NotHandled => {
                let program = &argv[0];
                let args = &argv[1..];
                if background {
                    let mut command = Command::new(program);
                    command.args(args);
                    command.envs(std::env::vars());
                    let child = command.spawn()
                        .map_err(|e| ShellError::ExecFailed { program: program.clone(), message: e.to_string() })?;
                    let cmd_str = format!("{} {}", program, args.join(" "));
                    let job_id = self.jobs.add_job(cmd_str, child);
                    println!("[{}] {}", job_id, self.jobs.list_jobs().last().unwrap().command);
                    Ok(0)
                } else {
                    match run_external_command(program, args) {
                        Ok(code) => Ok(code),
                        Err(e) => {
                            diagnostics::print_error(&e);
                            match e {
                                crate::error::ShellError::CommandNotFound { .. } => Ok(127),
                                crate::error::ShellError::ExecFailed { .. } => Ok(126),
                                _ => Ok(1),
                            }
                        }
                    }
                }
            }
        }
    }

    fn execute_pipe(&mut self, left: &CommandPart, right: &CommandPart) -> Result<i32, ShellError> {
        let left_output = self.capture_output(left)?;
        self.execute_with_input(right, &left_output)
    }

    fn execute_redirect_out(&mut self, cmd: &CommandPart, file: &str, append: bool) -> Result<i32, ShellError> {
        let output = self.capture_output(cmd)?;
        let mut file_handle = OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(file)
            .map_err(|e| ShellError::Other(format!("cannot open {}: {}", file, e)))?;
        file_handle.write_all(&output)
            .map_err(|e| ShellError::Other(format!("cannot write to {}: {}", file, e)))?;
        Ok(0)
    }

    fn execute_redirect_in(&mut self, cmd: &CommandPart, file: &str) -> Result<i32, ShellError> {
        let mut file_handle = std::fs::File::open(file)
            .map_err(|e| ShellError::Other(format!("cannot open {}: {}", file, e)))?;
        let mut input = Vec::new();
        file_handle.read_to_end(&mut input)
            .map_err(|e| ShellError::Other(format!("cannot read from {}: {}", file, e)))?;
        self.execute_with_input(cmd, &input)
    }

    fn execute_chain(&mut self, left: &CommandPart, right: &CommandPart, and: bool) -> Result<i32, ShellError> {
        let left_status = self.execute_command(left)?;
        let should_run_right = if and {
            left_status == 0  // &&: run if left succeeded
        } else {
            left_status != 0  // ||: run if left failed
        };
        if should_run_right {
            self.execute_command(right)
        } else {
            Ok(left_status)
        }
    }

    fn capture_output(&mut self, cmd: &CommandPart) -> Result<Vec<u8>, ShellError> {
        match cmd {
            CommandPart::Simple { argv, background: _ } => {
                if argv.is_empty() {
                    return Ok(Vec::new());
                }
                match try_handle_builtin(argv)? {
                    BuiltinResult::Handled(_) => Ok(Vec::new()),
                    BuiltinResult::HandledWithOutput(_, output) => Ok(output),
                    BuiltinResult::NotHandled => {
                        let program = &argv[0];
                        let args = &argv[1..];
                        let mut command = Command::new(program);
                        command.args(args);
                        command.envs(std::env::vars());
                        command.stdout(Stdio::piped());
                        command.stderr(Stdio::inherit());
                        let output = command.output()
                            .map_err(|e| ShellError::ExecFailed { program: program.clone(), message: e.to_string() })?;
                        if !output.status.success() {
                            return Err(ShellError::Other(format!("command failed with status {}", output.status.code().unwrap_or(-1))));
                        }
                        Ok(output.stdout)
                    }
                }
            }
            CommandPart::Pipe { left, right } => {
                let left_out = self.capture_output(left)?;
                let mut command = self.build_command_for_pipe(right, &left_out)?;
                command.stdout(Stdio::piped());
                command.stderr(Stdio::inherit());
                let output = command.output()?;
                if !output.status.success() {
                    return Err(ShellError::Other(format!("command failed with status {}", output.status.code().unwrap_or(-1))));
                }
                Ok(output.stdout)
            }
            CommandPart::RedirectOut { cmd, .. } | CommandPart::RedirectIn { cmd, .. } => {
                self.capture_output(cmd)
            }
            CommandPart::Chain { left, .. } => {
                self.capture_output(left)
            }
        }
    }

    fn execute_with_input(&mut self, cmd: &CommandPart, input: &[u8]) -> Result<i32, ShellError> {
        match cmd {
            CommandPart::Simple { argv, background: _ } => {
                if argv.is_empty() {
                    return Ok(0);
                }
                match try_handle_builtin(argv)? {
                    BuiltinResult::Handled(status) => Ok(status),
                    BuiltinResult::HandledWithOutput(status, _) => Ok(status),
                    BuiltinResult::NotHandled => {
                        let program = &argv[0];
                        let args = &argv[1..];
                        let mut command = Command::new(program);
                        command.args(args);
                        command.envs(std::env::vars());
                        command.stdin(Stdio::piped());
                        command.stdout(Stdio::inherit());
                        command.stderr(Stdio::inherit());
                        let mut child = command.spawn()
                            .map_err(|e| ShellError::ExecFailed { program: program.clone(), message: e.to_string() })?;
                        if let Some(mut stdin) = child.stdin.take() {
                            stdin.write_all(input)
                                .map_err(|e| ShellError::Other(format!("pipe write error: {}", e)))?;
                        }
                        let status = child.wait()
                            .map_err(|e| ShellError::ExecFailed { program: program.clone(), message: e.to_string() })?;
                        Ok(status.code().unwrap_or(1))
                    }
                }
            }
            CommandPart::Pipe { left, right } => {
                let left_out = self.capture_output(left)?;
                self.execute_with_input(right, &left_out)
            }
            CommandPart::RedirectOut { cmd, .. } | CommandPart::RedirectIn { cmd, .. } => {
                self.execute_with_input(cmd, input)
            }
            CommandPart::Chain { left, right, and } => {
                let left_status = self.execute_with_input(left, input)?;
                let should_run = if *and { left_status == 0 } else { left_status != 0 };
                if should_run {
                    self.execute_with_input(right, input)
                } else {
                    Ok(left_status)
                }
            }
        }
    }

    fn build_command_for_pipe(&self, cmd: &CommandPart, _input: &[u8]) -> Result<Command, ShellError> {
        match cmd {
            CommandPart::Simple { argv, background: _ } => {
                if argv.is_empty() {
                    return Err(ShellError::Other("empty command in pipe".to_string()));
                }
                let program = &argv[0];
                let args = &argv[1..];
                let mut command = Command::new(program);
                command.args(args);
                command.envs(std::env::vars());
                command.stdin(Stdio::piped());
                Ok(command)
            }
            _ => Err(ShellError::Other("complex commands in pipes not fully supported".to_string())),
        }
    }

    fn execute_with_timing(&mut self, argv: &[String], background: bool) -> Result<(i32, TimingInfo), ShellError> {
        if argv.is_empty() {
            return Ok((0, TimingInfo { real: 0.0, user: 0.0, system: 0.0 }));
        }

        let start = Instant::now();
        
        let is_external = match try_handle_builtin(argv)? {
            BuiltinResult::NotHandled => true,
            _ => false,
        };

        let (status, user_time, system_time) = if is_external && !background {
            self.execute_external_with_timing(argv)?
        } else {
            let status = if background {
                self.execute_simple(argv, background)?
            } else {
                match try_handle_builtin(argv)? {
                    BuiltinResult::Handled(s) => s,
                    BuiltinResult::HandledWithOutput(s, _) => s,
                    BuiltinResult::NotHandled => {
                        run_external_command(&argv[0], &argv[1..])
                            .map_err(|e| {
                                diagnostics::print_error(&e);
                                match e {
                                    crate::error::ShellError::CommandNotFound { .. } => ShellError::CommandNotFound { program: argv[0].clone() },
                                    _ => e,
                                }
                            })
                            .unwrap_or(1)
                    }
                }
            };
            (status, 0.0, 0.0)
        };

        let elapsed = start.elapsed();
        let real_time = elapsed.as_secs_f64();

        Ok((status, TimingInfo {
            real: real_time,
            user: user_time,
            system: system_time,
        }))
    }

    fn execute_external_with_timing(&mut self, argv: &[String]) -> Result<(i32, f64, f64), ShellError> {
        let program = &argv[0];
        let args = &argv[1..];
        
        let mut command = Command::new(program);
        command.args(args);
        command.envs(std::env::vars());
        command.stdin(Stdio::inherit());
        command.stdout(Stdio::inherit());
        command.stderr(Stdio::inherit());

        let mut child = command.spawn()
            .map_err(|e| {
                use std::io::ErrorKind;
                match e.kind() {
                    ErrorKind::NotFound => ShellError::CommandNotFound { program: program.clone() },
                    _ => ShellError::ExecFailed { program: program.clone(), message: e.to_string() },
                }
            })?;
        
        let pid = child.id() as i32;
        
        #[cfg(target_os = "linux")]
        {
            use libc::{wait4, rusage};
            let mut rusage: rusage = unsafe { std::mem::zeroed() };
            let mut status: i32 = 0;
            
            let result = unsafe { wait4(pid, &mut status, 0, &mut rusage) };
            
            if result == pid {
                let user_time = rusage.ru_utime.tv_sec as f64 + rusage.ru_utime.tv_usec as f64 / 1_000_000.0;
                let system_time = rusage.ru_stime.tv_sec as f64 + rusage.ru_stime.tv_usec as f64 / 1_000_000.0;
                let exit_code = if libc::WIFEXITED(status) {
                    libc::WEXITSTATUS(status)
                } else {
                    1
                };
                return Ok((exit_code, user_time, system_time));
            }
        }
        
        let status = child.wait()
            .map_err(|e| ShellError::ExecFailed { program: program.clone(), message: e.to_string() })?;
        
        let exit_code = status.code().unwrap_or(1);
        
        let mut user_time = 0.0;
        let mut system_time = 0.0;
        for _ in 0..3 {
            if let Some((u, s)) = get_child_process_times(pid as u32) {
                user_time = u;
                system_time = s;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        
        Ok((exit_code, user_time, system_time))
    }

    fn display_detailed_timing(&self, timing: &TimingInfo) {
        use colored::Colorize;
        
        let format_time = |t: f64| {
            if t < 0.001 {
                format!("{:.3}m", t * 1000.0)
            } else if t < 1.0 {
                format!("{:.3}s", t)
            } else {
                format!("{:.2}s", t)
            }
        };

        eprintln!("\n{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());
        eprintln!("{}", "  Timing Information".bold());
        eprintln!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());
        eprintln!("  {}  {}", "Real:".truecolor(150, 255, 180).bold(), format_time(timing.real));
        
        if timing.user > 0.0 || timing.system > 0.0 {
            eprintln!("  {}  {}", "User:".truecolor(140, 180, 255).bold(), format_time(timing.user));
            eprintln!("  {}  {}", "Sys: ".truecolor(255, 200, 120).bold(), format_time(timing.system));
            
            let total_cpu = timing.user + timing.system;
            if total_cpu > 0.0 {
                let cpu_percent = (total_cpu / timing.real * 100.0).min(100.0);
                eprintln!("  {}  {:.1}%", "CPU: ".truecolor(200, 150, 255).bold(), cpu_percent);
            }
        }
        eprintln!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());
    }
}

fn get_child_process_times(pid: u32) -> Option<(f64, f64)> {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        let stat_path = format!("/proc/{}/stat", pid);
        if let Ok(stat_content) = fs::read_to_string(&stat_path) {
            let comm_end = stat_content.find(')')?;
            let rest = &stat_content[comm_end + 1..];
            let fields: Vec<&str> = rest.trim_start().split_whitespace().collect();
            
            if fields.len() >= 14 {
                if let (Ok(utime_ticks), Ok(stime_ticks)) = (
                    fields[12].parse::<u64>(),
                    fields[13].parse::<u64>(),
                ) {
                    unsafe extern "C" {
                        fn sysconf(name: i32) -> i64;
                    }
                    const _SC_CLK_TCK: i32 = 2;
                    let clock_ticks = unsafe { sysconf(_SC_CLK_TCK) } as f64;
                    if clock_ticks > 0.0 {
                        let user_time = utime_ticks as f64 / clock_ticks;
                        let system_time = stime_ticks as f64 / clock_ticks;
                        return Some((user_time, system_time));
                    }
                }
            }
        }
    }
    None
}

