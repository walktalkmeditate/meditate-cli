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

pub fn config_dir() -> Result<PathBuf, PathError> {
    resolve("XDG_CONFIG_HOME", |dirs| dirs.config_dir().to_path_buf())
}

pub fn data_dir() -> Result<PathBuf, PathError> {
    resolve("XDG_DATA_HOME", |dirs| dirs.data_dir().to_path_buf())
}

fn resolve(
    env_var: &str,
    fallback: impl Fn(&directories::ProjectDirs) -> PathBuf,
) -> Result<PathBuf, PathError> {
    if let Some(base) = base_from_env(env_var) {
        validate_base_dir(&base)?;
        return Ok(base.join("meditate"));
    }
    let dirs = directories::ProjectDirs::from("", "", "meditate").ok_or(PathError::NoHome)?;
    Ok(fallback(&dirs))
}

fn base_from_env(env_var: &str) -> Option<PathBuf> {
    std::env::var_os(env_var)
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}
