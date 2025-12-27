use crate::error::ShellError;

#[derive(Debug, Clone)]
pub enum CommandPart {
    Simple { argv: Vec<String>, background: bool },
    Pipe { left: Box<CommandPart>, right: Box<CommandPart> },
    RedirectOut { cmd: Box<CommandPart>, file: String, append: bool },
    RedirectIn { cmd: Box<CommandPart>, file: String },
    Chain { left: Box<CommandPart>, right: Box<CommandPart>, and: bool },
}

pub fn parse_command_line(input: &str) -> Result<CommandPart, ShellError> {
    let tokens = tokenize(input)?;
    parse_tokens(&tokens)
}

fn tokenize(input: &str) -> Result<Vec<Token>, ShellError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(ch) = chars.next() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            ' ' | '\t' if !in_single && !in_double => {
                if !current.is_empty() {
                    tokens.push(Token::Word(current.clone()));
                    current.clear();
                }
            }
            '|' if !in_single && !in_double => {
                if !current.is_empty() {
                    tokens.push(Token::Word(current.clone()));
                    current.clear();
                }
                if chars.peek() == Some(&'|') {
                    chars.next();
                    tokens.push(Token::Or);
                } else {
                    tokens.push(Token::Pipe);
                }
            }
            '&' if !in_single && !in_double => {
                if !current.is_empty() {
                    tokens.push(Token::Word(current.clone()));
                    current.clear();
                }
                if chars.peek() == Some(&'&') {
                    chars.next();
                    tokens.push(Token::And);
                } else {
                    tokens.push(Token::Background);
                }
            }
            '>' if !in_single && !in_double => {
                if !current.is_empty() {
                    tokens.push(Token::Word(current.clone()));
                    current.clear();
                }
                if chars.peek() == Some(&'>') {
                    chars.next();
                    tokens.push(Token::RedirectAppend);
                } else {
                    tokens.push(Token::RedirectOut);
                }
            }
            '<' if !in_single && !in_double => {
                if !current.is_empty() {
                    tokens.push(Token::Word(current.clone()));
                    current.clear();
                }
                tokens.push(Token::RedirectIn);
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(Token::Word(current));
    }

    Ok(tokens)
}

#[derive(Debug, Clone)]
enum Token {
    Word(String),
    Pipe,
    RedirectOut,
    RedirectAppend,
    RedirectIn,
    And,
    Or,
    Background,
}

fn parse_tokens(tokens: &[Token]) -> Result<CommandPart, ShellError> {
    if tokens.is_empty() {
        return Err(ShellError::Other("empty command".to_string()));
    }

    parse_chain(tokens)
}

fn parse_chain(tokens: &[Token]) -> Result<CommandPart, ShellError> {
    let mut parts = Vec::new();
    let mut ops = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        let (part, next_i) = parse_pipe(&tokens[i..])?;
        parts.push(part);
        i = next_i;

        if i < tokens.len() {
            match &tokens[i] {
                Token::And => {
                    ops.push(true);
                    i += 1;
                }
                Token::Or => {
                    ops.push(false);
                    i += 1;
                }
                _ => break,
            }
        }
    }

    if parts.is_empty() {
        return Err(ShellError::Other("empty command".to_string()));
    }

    let mut result = parts.remove(0);
    for (op, right) in ops.into_iter().zip(parts) {
        result = CommandPart::Chain {
            left: Box::new(result),
            right: Box::new(right),
            and: op,
        };
    }

    Ok(result)
}

fn parse_pipe(tokens: &[Token]) -> Result<(CommandPart, usize), ShellError> {
    let mut parts = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        let (part, next_i) = parse_redirect(&tokens[i..])?;
        parts.push(part);
        i += next_i;

        if i < tokens.len() && matches!(tokens[i], Token::Pipe) {
            i += 1;
        } else {
            break;
        }
    }

    if parts.is_empty() {
        return Err(ShellError::Other("empty command".to_string()));
    }

    let mut result = parts.remove(0);
    for part in parts {
        result = CommandPart::Pipe {
            left: Box::new(result),
            right: Box::new(part),
        };
    }

    Ok((result, i))
}

fn parse_redirect(tokens: &[Token]) -> Result<(CommandPart, usize), ShellError> {
    if tokens.is_empty() {
        return Err(ShellError::Other("empty command".to_string()));
    }

    let (cmd, mut i) = parse_simple(&tokens)?;

    while i < tokens.len() {
        match &tokens[i] {
            Token::RedirectOut => {
                i += 1;
                if i >= tokens.len() {
                    return Err(ShellError::Other("redirect output: missing filename".to_string()));
                }
                if let Token::Word(file) = &tokens[i] {
                    let file = expand_tilde(file);
                    return Ok((
                        CommandPart::RedirectOut {
                            cmd: Box::new(cmd),
                            file,
                            append: false,
                        },
                        i + 1,
                    ));
                } else {
                    return Err(ShellError::Other("redirect output: expected filename".to_string()));
                }
            }
            Token::RedirectAppend => {
                i += 1;
                if i >= tokens.len() {
                    return Err(ShellError::Other("redirect append: missing filename".to_string()));
                }
                if let Token::Word(file) = &tokens[i] {
                    let file = expand_tilde(file);
                    return Ok((
                        CommandPart::RedirectOut {
                            cmd: Box::new(cmd),
                            file,
                            append: true,
                        },
                        i + 1,
                    ));
                } else {
                    return Err(ShellError::Other("redirect append: expected filename".to_string()));
                }
            }
            Token::RedirectIn => {
                i += 1;
                if i >= tokens.len() {
                    return Err(ShellError::Other("redirect input: missing filename".to_string()));
                }
                if let Token::Word(file) = &tokens[i] {
                    let file = expand_tilde(file);
                    return Ok((
                        CommandPart::RedirectIn {
                            cmd: Box::new(cmd),
                            file,
                        },
                        i + 1,
                    ));
                } else {
                    return Err(ShellError::Other("redirect input: expected filename".to_string()));
                }
            }
            _ => break,
        }
    }

    Ok((cmd, i))
}

fn parse_simple(tokens: &[Token]) -> Result<(CommandPart, usize), ShellError> {
    if tokens.is_empty() {
        return Err(ShellError::Other("empty command".to_string()));
    }

    let mut argv = Vec::new();
    let mut i = 0;
    let mut background = false;

    while i < tokens.len() {
        match &tokens[i] {
            Token::Word(word) => {
                let expanded = expand_word_with_subst(word)?;
                let globbed = expand_glob(&expanded);
                if globbed.is_empty() {
                    argv.push(expanded);
                } else {
                    argv.extend(globbed);
                }
                i += 1;
            }
            Token::Background => {
                background = true;
                i += 1;
                break;
            }
            _ => break,
        }
    }

    if argv.is_empty() {
        return Err(ShellError::Other("empty command".to_string()));
    }

    Ok((CommandPart::Simple { argv, background }, i))
}

fn expand_tilde(input: &str) -> String {
    use std::env;
    
    let home = match env::var("HOME") {
        Ok(h) => h,
        Err(_) => return input.to_string(),
    };

    if input == "~" {
        home
    } else if input.starts_with("~/") {
        format!("{}/{}", home, &input[2..])
    } else {
        input.to_string()
    }
}

fn expand_word_with_subst(word: &str) -> Result<String, ShellError> {
    let s = expand_tilde(word);
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    
    while let Some(ch) = chars.next() {
        if ch == '$' {
            if let Some('(') = chars.peek().copied() {
                chars.next();
                let mut cmd_str = String::new();
                let mut depth = 1;
                while let Some(c) = chars.next() {
                    if c == '(' { depth += 1; }
                    if c == ')' { 
                        depth -= 1;
                        if depth == 0 { break; }
                    }
                    cmd_str.push(c);
                }
                let subst_output = execute_command_subst(&cmd_str)?;
                out.push_str(&subst_output);
            } else if let Some('{') = chars.peek().copied() {
                chars.next();
                let mut name = String::new();
                while let Some(c) = chars.next() {
                    if c == '}' { break; }
                    name.push(c);
                }
                if let Ok(val) = std::env::var(&name) { out.push_str(&val); }
            } else {
                let mut name = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' { name.push(c); chars.next(); } else { break; }
                }
                if !name.is_empty() {
                    if let Ok(val) = std::env::var(&name) { out.push_str(&val); }
                } else {
                    out.push('$');
                }
            }
        } else if ch == '`' {
            let mut cmd_str = String::new();
            while let Some(c) = chars.next() {
                if c == '`' { break; }
                cmd_str.push(c);
            }
            let subst_output = execute_command_subst(&cmd_str)?;
            out.push_str(&subst_output);
        } else {
            out.push(ch);
        }
    }
    Ok(out)
}

fn execute_command_subst(cmd: &str) -> Result<String, ShellError> {
    use std::process::Command;
    let output = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .map_err(|_| ShellError::Other("command substitution failed".to_string()))?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(text)
}

fn expand_glob(word: &str) -> Vec<String> {
    if !(word.contains('*') || word.contains('?') || word.contains('[')) { return Vec::new(); }
    let mut out = Vec::new();
    if let Ok(paths) = glob::glob(word) {
        for entry in paths.flatten() {
            if let Some(s) = entry.to_str() { out.push(s.to_string()); }
        }
    }
    out
}

