#[cfg(target_os = "macos")]
use directories_next::BaseDirs;

#[cfg(target_os = "macos")]
use std::{
    collections::HashSet,
    env,
    fs,
    path::PathBuf,
};

/// Locate binaries in common macOS installation directories.
#[cfg(target_os = "macos")]
pub fn locate_macos_binary(names: &[&str]) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();

    let mut push_dir = |dir: PathBuf| {
        if seen.insert(dir.clone()) {
            dirs.push(dir);
        }
    };

    if let Some(path_os) = env::var_os("PATH") {
        for entry in env::split_paths(&path_os) {
            push_dir(entry);
        }
    }

    if let Ok(content) = fs::read_to_string("/etc/paths") {
        for line in content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            push_dir(PathBuf::from(line));
        }
    }

    if let Ok(entries) = fs::read_dir("/etc/paths.d") {
        for entry in entries.flatten() {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                for line in content
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                {
                    push_dir(PathBuf::from(line));
                }
            }
        }
    }

    for dir in [
        "/opt/homebrew/bin",
        "/opt/homebrew/sbin",
        "/opt/homebrew/opt/ffmpeg/bin",
        "/opt/homebrew/opt/yt-dlp/bin",
        "/opt/local/bin",
        "/opt/local/sbin",
        "/usr/local/bin",
        "/usr/local/sbin",
        "/usr/local/opt/ffmpeg/bin",
        "/usr/local/opt/yt-dlp/bin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ] {
        push_dir(PathBuf::from(dir));
    }

    if let Some(base_dirs) = BaseDirs::new() {
        let home = base_dirs.home_dir();
        for relative in [
            "bin",
            ".local/bin",
            "Library/Python/3.7/bin",
            "Library/Python/3.8/bin",
            "Library/Python/3.9/bin",
            "Library/Python/3.10/bin",
            "Library/Python/3.11/bin",
            "Library/Python/3.12/bin",
            "Library/Application Support/Homebrew/bin",
        ] {
            push_dir(home.join(relative));
        }
    }

    if let Ok(pipx_dir) = env::var("PIPX_BIN_DIR") {
        push_dir(PathBuf::from(pipx_dir));
    }

    for dir in dirs.into_iter() {
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

#[cfg(not(target_os = "macos"))]
pub fn locate_macos_binary(_names: &[&str]) -> Option<std::path::PathBuf> {
    None
}
