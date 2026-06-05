use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub enum PathError {
    NotAbsolute(PathBuf),
    Traversal(PathBuf),
    Symlink(PathBuf),
    NoHome,
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathError::NotAbsolute(p) => write!(f, "directory is not absolute: {}", p.display()),
            PathError::Traversal(p) => {
                write!(f, "directory traverses parent directories: {}", p.display())
            }
            PathError::Symlink(p) => write!(f, "directory is a symlink: {}", p.display()),
            PathError::NoHome => write!(f, "could not determine a home directory"),
        }
    }
}

impl std::error::Error for PathError {}

/// Reject base directories that are relative, contain `..`, or are symlinks.
///
/// These guards run before any read or write so a hostile `XDG_*` value cannot
/// redirect meditate's config or state into an unexpected location.
pub fn validate_base_dir(dir: &Path) -> Result<(), PathError> {
    if !dir.is_absolute() {
        return Err(PathError::NotAbsolute(dir.to_path_buf()));
    }
    if dir.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(PathError::Traversal(dir.to_path_buf()));
    }
    if let Ok(meta) = std::fs::symlink_metadata(dir) {
        if meta.file_type().is_symlink() {
            return Err(PathError::Symlink(dir.to_path_buf()));
        }
    }
    Ok(())
}

/// Where `config.toml` lives: `$XDG_CONFIG_HOME/meditate`, else `~/.config/meditate`
/// on macOS and Linux, else the OS-native config dir on Windows (`%APPDATA%`).
pub fn config_dir() -> Result<PathBuf, PathError> {
    resolve("XDG_CONFIG_HOME", ".config", |dirs| {
        dirs.config_dir().to_path_buf()
    })
}

/// Where state, streak, and downloaded packs live: `$XDG_DATA_HOME/meditate`, else
/// `~/.local/share/meditate` on macOS and Linux, else the OS-native data dir on Windows.
pub fn data_dir() -> Result<PathBuf, PathError> {
    resolve("XDG_DATA_HOME", ".local/share", |dirs| {
        dirs.data_dir().to_path_buf()
    })
}

fn resolve(
    env_var: &str,
    home_subdir: &str,
    native: impl Fn(&directories::ProjectDirs) -> PathBuf,
) -> Result<PathBuf, PathError> {
    if let Some(base) = base_from_env(env_var) {
        validate_base_dir(&base)?;
        return Ok(base.join("meditate"));
    }
    // A terminal tool belongs under the XDG-style home dirs on macOS and Linux —
    // dotfile-friendly and what CLI users reach for — rather than macOS's
    // Application Support. Windows falls through to its native location.
    if let Some(dir) = home_based(home_subdir) {
        return Ok(dir);
    }
    let dirs = directories::ProjectDirs::from("", "", "meditate").ok_or(PathError::NoHome)?;
    Ok(native(&dirs))
}

/// `~/<subdir>/meditate` on Unix (macOS, Linux). `None` elsewhere, so Windows
/// keeps its OS-native dirs.
#[cfg(unix)]
fn home_based(subdir: &str) -> Option<PathBuf> {
    let mut dir = directories::BaseDirs::new()?.home_dir().to_path_buf();
    for part in subdir.split('/') {
        dir.push(part);
    }
    dir.push("meditate");
    Some(dir)
}

#[cfg(not(unix))]
fn home_based(_subdir: &str) -> Option<PathBuf> {
    None
}

fn base_from_env(env_var: &str) -> Option<PathBuf> {
    std::env::var_os(env_var)
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

/// Move config, state, streak, and packs from the OS-native dirs meditate used
/// through v0.2.1 (on macOS, `~/Library/Application Support/meditate`) into the
/// `~/.config` and `~/.local/share` homes, once, so an upgrade keeps a streak and
/// any downloaded packs. Best-effort and idempotent: anything already migrated,
/// missing, or unchanged (Linux, where old and new paths coincide) is left alone.
pub fn migrate_legacy_dirs() {
    let Some(legacy) = directories::ProjectDirs::from("", "", "meditate") else {
        return;
    };
    let (Ok(new_config), Ok(new_data)) = (config_dir(), data_dir()) else {
        return;
    };
    migrate_into(
        legacy.config_dir(),
        legacy.data_dir(),
        &new_config,
        &new_data,
    );
}

fn migrate_into(legacy_config: &Path, legacy_data: &Path, new_config: &Path, new_data: &Path) {
    migrate_entry(legacy_config, new_config, "config.toml");
    for name in ["state.toml", "streak.toml", "packs"] {
        migrate_entry(legacy_data, new_data, name);
    }
}

/// Move `old_dir/name` to `new_dir/name` when the source exists and the
/// destination does not. A no-op when the two resolve to the same path (Linux).
fn migrate_entry(old_dir: &Path, new_dir: &Path, name: &str) {
    let src = old_dir.join(name);
    let dst = new_dir.join(name);
    if src == dst || dst.exists() || !src.exists() {
        return;
    }
    if let Some(parent) = dst.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    let _ = std::fs::rename(&src, &dst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn migrate_fans_one_legacy_dir_into_config_and_data() {
        let root = tempdir().unwrap();
        // On macOS the legacy config and data dirs are the same folder.
        let legacy = root.path().join("legacy");
        fs::create_dir_all(legacy.join("packs/sounds")).unwrap();
        fs::write(legacy.join("config.toml"), "default_pattern = \"calm\"").unwrap();
        fs::write(legacy.join("state.toml"), "x = 1").unwrap();
        fs::write(legacy.join("streak.toml"), "current = 4").unwrap();
        fs::write(legacy.join("packs/sounds/rain.aac"), b"audio").unwrap();

        let new_config = root.path().join("config/meditate");
        let new_data = root.path().join("data/meditate");
        migrate_into(&legacy, &legacy, &new_config, &new_data);

        assert!(new_config.join("config.toml").exists());
        assert!(new_data.join("state.toml").exists());
        assert!(new_data.join("streak.toml").exists());
        assert_eq!(
            fs::read(new_data.join("packs/sounds/rain.aac")).unwrap(),
            b"audio"
        );
        // The originals are moved, not copied.
        assert!(!legacy.join("config.toml").exists());
        assert!(!legacy.join("packs").exists());
    }

    #[test]
    fn migrate_never_clobbers_existing_destination() {
        let root = tempdir().unwrap();
        let legacy = root.path().join("legacy");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("streak.toml"), "current = 1").unwrap();

        let new_data = root.path().join("data/meditate");
        fs::create_dir_all(&new_data).unwrap();
        fs::write(new_data.join("streak.toml"), "current = 99").unwrap();

        migrate_into(&legacy, &legacy, &root.path().join("config"), &new_data);

        // The newer streak survives; the legacy one is left where it was.
        assert_eq!(
            fs::read_to_string(new_data.join("streak.toml")).unwrap(),
            "current = 99"
        );
        assert!(legacy.join("streak.toml").exists());
    }

    #[test]
    fn migrate_is_a_noop_when_paths_coincide() {
        // Linux: legacy and new dirs are identical, so nothing moves or is lost.
        let root = tempdir().unwrap();
        let dir = root.path().join("meditate");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.toml"), "x = 1").unwrap();

        migrate_into(&dir, &dir, &dir, &dir);

        assert!(dir.join("config.toml").exists());
    }
}
