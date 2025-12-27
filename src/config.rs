use std::path::PathBuf;

fn ensure_dir(p: &PathBuf) -> std::io::Result<()> {
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

pub fn config_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let mut path = PathBuf::from(home);
    path.push(".config");
    path.push("squish");
    if std::fs::create_dir_all(&path).is_ok() {
        Some(path)
    } else {
        None
    }
}

pub fn history_file() -> Option<PathBuf> {
    let mut p = config_dir()?;
    p.push("history");
    if ensure_dir(&p).is_ok() { Some(p) } else { None }
}

pub fn dirfreq_file() -> Option<PathBuf> {
    let mut p = config_dir()?;
    p.push("dirfreq");
    if ensure_dir(&p).is_ok() { Some(p) } else { None }
}

pub fn alias_file() -> Option<PathBuf> {
    let mut p = config_dir()?;
    p.push("aliases");
    if ensure_dir(&p).is_ok() { Some(p) } else { None }
}

pub fn config_file() -> Option<PathBuf> {
    let mut p = config_dir()?;
    p.push("config");
    if ensure_dir(&p).is_ok() { Some(p) } else { None }
}





