//! Services shared by native targets: clock, logging and file-backed storage.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();
static QUIT: AtomicBool = AtomicBool::new(false);
static GRAB: AtomicBool = AtomicBool::new(false);

pub fn now_ms() -> f64 {
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// Directory that holds worlds, options, logs and screenshots.
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("RUSTCRAFT_HOME").map(PathBuf::from).or_else(|| {
        if cfg!(windows) {
            std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("Rustcraft Redefined"))
        } else {
            std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share"))).map(|p| p.join("rustcraft-redefined"))
        }
    });
    base.unwrap_or_else(|| PathBuf::from("rustcraft-data"))
}

pub fn log(msg: &str) {
    use std::io::Write;
    eprintln!("{msg}");
    let dir = data_dir();
    if std::fs::create_dir_all(&dir).is_ok() {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("latest.log")) {
            let _ = writeln!(f, "[{:>10.3}] {msg}", now_ms() / 1000.0);
        }
    }
}

/// Asks the window loop to capture or release the cursor.
pub fn set_cursor_grab(on: bool) {
    GRAB.store(on, Ordering::Relaxed);
}
pub fn cursor_grab_requested() -> bool {
    GRAB.load(Ordering::Relaxed)
}
pub fn request_quit() {
    QUIT.store(true, Ordering::Relaxed);
}
pub fn quit_requested() -> bool {
    QUIT.load(Ordering::Relaxed)
}

fn key_path(key: &str) -> Option<PathBuf> {
    // Keys are relative paths such as "worlds/New World/region/r.0.0.rcr"; reject anything that escapes.
    if key.split(['/', '\\']).any(|part| part == ".." || part.contains(':')) || key.starts_with('/') {
        return None;
    }
    Some(data_dir().join(key))
}

pub fn storage_read(key: &str) -> Option<Vec<u8>> {
    std::fs::read(key_path(key)?).ok()
}

/// Writes through a temporary file so a crash never leaves a half-written save.
pub fn storage_write(key: &str, data: &[u8]) {
    let Some(path) = key_path(key) else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, data).is_ok() && std::fs::rename(&tmp, &path).is_err() {
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::rename(&tmp, &path);
    }
}

pub fn storage_remove(key: &str) {
    if let Some(path) = key_path(key) {
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(path);
        } else {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Lists keys that start with `prefix` (a directory prefix ending in '/').
pub fn storage_list(prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(root) = key_path(prefix.trim_end_matches('/')) else { return out };
    fn walk(dir: &std::path::Path, prefix: &str, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let key = format!("{prefix}{name}");
            if e.path().is_dir() {
                walk(&e.path(), &format!("{key}/"), out);
            } else {
                out.push(key);
            }
        }
    }
    let p = if prefix.is_empty() || prefix.ends_with('/') { prefix.to_string() } else { format!("{prefix}/") };
    walk(&root, &p, &mut out);
    out.sort();
    out
}

/// Saves a user-facing file (screenshots) and logs where it went.
/// Registers a sound effect (16-bit mono samples) under an id.
pub fn sound_load(id: u32, samples: &[i16], rate: u32) {
    #[cfg(windows)]
    super::windows::sound_load(id, samples, rate);
    #[cfg(not(windows))]
    let _ = (id, samples, rate);
}
pub fn sound_play(id: u32, volume: f32, pitch: f32) {
    #[cfg(windows)]
    super::windows::sound_play(id, volume);
    #[cfg(not(windows))]
    let _ = (id, volume);
    let _ = pitch;
}
pub fn export_file(name: &str, data: &[u8]) {
    let dir = data_dir().join("screenshots");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(name);
    match std::fs::write(&path, data) {
        Ok(()) => log(&format!("Saved {}", path.display())),
        Err(e) => log(&format!("Could not save {}: {e}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_round_trip_and_path_safety() {
        let dir = std::env::temp_dir().join(format!("rc-test-{}", std::process::id()));
        std::env::set_var("RUSTCRAFT_HOME", &dir);
        storage_write("worlds/a/level.dat", b"hello");
        storage_write("worlds/a/region/r.0.0.rcr", b"x");
        assert_eq!(storage_read("worlds/a/level.dat").as_deref(), Some(&b"hello"[..]));
        let keys = storage_list("worlds/");
        assert_eq!(keys, vec!["worlds/a/level.dat".to_string(), "worlds/a/region/r.0.0.rcr".to_string()]);
        assert!(storage_read("../outside").is_none());
        storage_write("../outside", b"no");
        assert!(!dir.parent().unwrap().join("outside").exists());
        storage_remove("worlds/a");
        assert!(storage_list("worlds/").is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
