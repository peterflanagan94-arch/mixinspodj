//! Differences between macOS and Windows that the rest of the app shouldn't
//! need to know about: the home folder, where Traktor keeps its files, and how
//! Traktor's volume-plus-folders locations turn into real paths.

use std::path::PathBuf;

/// The user's home folder (`HOME` on macOS, `USERPROFILE` on Windows).
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Folders that may hold Traktor's collection and history files.
pub fn traktor_roots() -> Vec<PathBuf> {
    let Some(home) = home_dir() else { return Vec::new() };
    // OneDrive can move Documents and Music out of the home folder on Windows.
    ["Documents", "Music", "OneDrive/Documents", "OneDrive/Music"]
        .iter()
        .map(|sub| home.join(sub).join("Native Instruments"))
        .collect()
}

/// Builds a real path from the parts Traktor stores. `dir` uses "/" as the
/// separator and starts with one (`/Users/me/Music/`). On macOS the boot disk
/// is "Macintosh" and other disks mount under /Volumes; on Windows the volume
/// is a drive such as "C:".
pub fn traktor_path(volume: &str, dir: &str, file: &str, windows: bool) -> String {
    if windows {
        let joined = if volume.len() == 2 && volume.ends_with(':') {
            format!("{volume}{dir}{file}")
        } else {
            format!("{dir}{file}")
        };
        return joined.replace('/', "\\");
    }
    let base = match volume {
        "" | "Macintosh" | "Macintosh HD" => String::new(),
        other => format!("/Volumes/{other}"),
    };
    format!("{base}{dir}{file}")
}

/// [`traktor_path`] for the operating system the app is running on.
pub fn native_traktor_path(volume: &str, dir: &str, file: &str) -> String {
    traktor_path(volume, dir, file, cfg!(windows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_paths() {
        assert_eq!(traktor_path("Macintosh", "/Users/me/Music/", "a.mp3", false), "/Users/me/Music/a.mp3");
        assert_eq!(traktor_path("Live Drive", "/Music/", "a.mp3", false), "/Volumes/Live Drive/Music/a.mp3");
    }

    #[test]
    fn windows_paths_use_the_drive_and_backslashes() {
        assert_eq!(traktor_path("C:", "/Users/me/Music/", "a.mp3", true), "C:\\Users\\me\\Music\\a.mp3");
        assert_eq!(traktor_path("D:", "/DJ/", "b.flac", true), "D:\\DJ\\b.flac");
    }
}
