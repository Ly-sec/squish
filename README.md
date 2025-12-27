# squish

<div align="center">
  <img src="https://i.imgur.com/YU9tpQf.png" alt="squish logo" width="248"/>
  
  *squish your workflow into shape.*
  
  > **Note:** This is still a very early and rough version. Things may break, features may be missing, and the API may change. squish should not be used as a daily driver yet!
</div>

## Features

### Prompt

- Automatically detects and displays your Linux distribution logo
- Shows git branch and status in the prompt
- Powerline-style design with customizable colors
- Visual indicators for command success/failure

### Performance

- Automatic command timing for slow commands
- Directory frequency tracking
- Persistent command history
- Tab completion for commands and file paths

### Built-in Commands

- `cd` - Change directory
- `ll` - Enhanced directory listing
- `freqs` - View directory usage statistics
- `alias` / `unalias` - Manage command aliases
- `jobs` / `fg` / `bg` - Background job management
- `export` / `unset` - Environment variable management
- `time` - Measure command execution time
- `help` - Built-in help system

### Additional Features

- Command aliasing
- Background job control
- Pipes and redirection support
- Configurable via `~/.config/squish/config`
- Autostart commands on shell launch

## Installation

### Requirements

- Rust 1.70 or later
- Terminal with Nerd Fonts support (for distro icons)

### Build from Source

```bash
cargo build --release
```

The binary will be located at `target/release/squish`.

### Set as Default Shell

```bash
# Add squish to /etc/shells
echo "/path/to/squish" | sudo tee -a /etc/shells

# Change your default shell
chsh -s /path/to/squish
```

## Configuration

The configuration file is located at `~/.config/squish/config`.

### Example Configuration

```bash
# Command timing
show_timing=true
timing_threshold_ms=50
fancy_mode=true

# Prompt colors
prompt.distro_text=black
prompt.distro_bg=bright_yellow
prompt.user_host_text=black
prompt.user_host_bg=white
prompt.dir_text=white
prompt.dir_bg=blue
prompt.git_text=white
prompt.git_bg=green
prompt.arrow_success=green
prompt.arrow_error=red

# Autostart commands
autostart=neofetch
autostart=echo "Welcome to squish"
```

### Available Colors

**Basic colors:** `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`

**Bright colors:** `bright_black`, `bright_red`, `bright_green`, `bright_yellow`, `bright_blue`, `bright_magenta`, `bright_cyan`, `bright_white`

**RGB colors:** `255,220,100` or `255 220 100`

## Usage

### Basic Commands

```bash
# Change directory
cd ~/projects

# List directory with details
ll /usr/bin

# View directory frequency statistics
freqs

# Create an alias
alias ll='ls -lah'
```

### Job Control

```bash
# Run command in background
sleep 10 &

# List background jobs
jobs

# Bring job to foreground
fg 1
```

### Advanced Usage

```bash
# Pipes and redirection
cat file.txt | grep "pattern" > output.txt

# Command chaining
cd /tmp && ls -la || echo "Failed"

# Time command execution
time find / -name "*.rs"
```

## Dependencies

- `rustyline` - Line editing and history
- `colored` - Terminal colors
- `chrono` - Time and date handling
- `humansize` - Human-readable file sizes
- `which` - Find executables in PATH
- `glob` - Pattern matching
- `libc` - System calls

## License

MIT License

