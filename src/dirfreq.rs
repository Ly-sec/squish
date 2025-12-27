use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use crate::config;

fn store_path() -> Option<PathBuf> { config::dirfreq_file() }

pub fn load_freqs() -> HashMap<String, u64> {
    let mut map = HashMap::new();
    let Some(path) = store_path() else { return map; };
    let file = match OpenOptions::new().read(true).open(&path) {
        Ok(f) => f,
        Err(_) => return map,
    };
    let reader = BufReader::new(file);
    for line in reader.lines().flatten() {
        if let Some((path, count)) = line.rsplit_once('\t') {
            if let Ok(n) = count.parse::<u64>() {
                map.insert(path.to_string(), n);
            }
        }
    }
    map
}

pub fn increment_dir_usage(dir: &Path) {
    let abs = match dir.canonicalize() {
        Ok(p) => p,
        Err(_) => dir.to_path_buf(),
    };
    let key = abs.to_string_lossy().to_string();
    let mut map = load_freqs();
    let entry = map.entry(key).or_insert(0);
    *entry = entry.saturating_add(1);
    let _ = save_freqs(&map);
}

pub fn get_count(path: &Path) -> u64 {
    let abs = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => path.to_path_buf(),
    };
    let key = abs.to_string_lossy().to_string();
    let map = load_freqs();
    map.get(&key).copied().unwrap_or(0)
}

fn save_freqs(map: &HashMap<String, u64>) -> std::io::Result<()> {
    if let Some(path) = store_path() {
        // ensure parent exists (HOME should)
        let mut tmp = path.clone();
        tmp.set_extension("tmp");
        let mut f = OpenOptions::new().create(true).write(true).truncate(true).open(&tmp)?;
        for (k, v) in map {
            writeln!(f, "{}\t{}", k, v)?;
        }
        f.flush()?;
        fs::rename(tmp, path)?;
    }
    Ok(())
}


