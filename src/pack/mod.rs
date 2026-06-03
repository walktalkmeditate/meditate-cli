pub mod soundscape;

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Base URL of the Pilgrim audio CDN. **Confirm this against the live manifest**
/// — the manifest is expected at `{BASE}/manifest.json` and each asset at
/// `{BASE}/{r2Key}`.
pub const DEFAULT_BASE_URL: &str = "https://cdn.pilgrimapp.org/audio";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetKind {
    Soundscape,
    Voice,
    Bell,
}

impl AssetKind {
    pub fn dir(self) -> &'static str {
        match self {
            AssetKind::Soundscape => "soundscapes",
            AssetKind::Voice => "voices",
            AssetKind::Bell => "bells",
        }
    }

    pub fn from_arg(arg: &str) -> Option<AssetKind> {
        match arg {
            "soundscape" | "soundscapes" => Some(AssetKind::Soundscape),
            "voice" | "voices" => Some(AssetKind::Voice),
            "bell" | "bells" => Some(AssetKind::Bell),
            _ => None,
        }
    }
}

/// One downloadable asset. Field names mirror Pilgrim's `AudioAsset` schema.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AudioAsset {
    pub id: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(rename = "displayName", default)]
    pub display_name: String,
    #[serde(rename = "durationSec", default)]
    pub duration_sec: f64,
    #[serde(rename = "fileSizeBytes")]
    pub file_size_bytes: u64,
    #[serde(rename = "r2Key")]
    pub r2_key: String,
    #[serde(rename = "meditationPrompts", default)]
    pub meditation_prompts: Vec<MeditationPrompt>,
}

/// A single meditation voice prompt (consumed by the voice scheduler). Walk
/// prompts are absent here by construction — only the meditation set is modeled.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MeditationPrompt {
    pub id: String,
    #[serde(rename = "r2Key")]
    pub r2_key: String,
    #[serde(default)]
    pub phase: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub soundscapes: Vec<AudioAsset>,
    #[serde(default)]
    pub voices: Vec<AudioAsset>,
    #[serde(default)]
    pub bells: Vec<AudioAsset>,
}

impl Manifest {
    pub fn assets_for(&self, kind: AssetKind) -> &[AudioAsset] {
        match kind {
            AssetKind::Soundscape => &self.soundscapes,
            AssetKind::Voice => &self.voices,
            AssetKind::Bell => &self.bells,
        }
    }
}

#[derive(Debug)]
pub enum PackError {
    Io(std::io::Error),
    Http(String),
    Manifest(String),
    UnknownAsset(String),
    UnsafeName(String),
    SizeMismatch { expected: u64, actual: u64 },
    NotAudio,
}

impl std::fmt::Display for PackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackError::Io(e) => write!(f, "{e}"),
            PackError::Http(e) => write!(f, "download failed: {e}"),
            PackError::Manifest(e) => write!(f, "could not read the pack manifest: {e}"),
            PackError::UnknownAsset(id) => write!(f, "no pack named '{id}'"),
            PackError::UnsafeName(id) => write!(f, "refusing unsafe asset name '{id}'"),
            PackError::SizeMismatch { expected, actual } => {
                write!(f, "size mismatch: expected {expected} bytes, got {actual}")
            }
            PackError::NotAudio => write!(f, "downloaded file is not recognizable audio"),
        }
    }
}

impl std::error::Error for PackError {}

impl From<std::io::Error> for PackError {
    fn from(err: std::io::Error) -> PackError {
        PackError::Io(err)
    }
}

/// The network seam, so download orchestration is testable without a network.
pub trait Fetcher {
    fn get(&self, url: &str) -> Result<Vec<u8>, PackError>;
}

/// Validate a single path component against an allowlist (no separators, no
/// `..`), the defense against path-traversal and zip-slip via manifest names.
pub fn safe_component(name: &str) -> Option<String> {
    let ok = !name.is_empty()
        && name.len() <= 128
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-');
    ok.then(|| name.to_string())
}

/// Validate a manifest `r2Key` before it is used to build a download URL:
/// relative, no `..`/`.`/empty/absolute segments, allowlisted characters
/// (forward slashes permitted, unlike `safe_component`).
pub fn safe_r2_key(key: &str) -> bool {
    !key.is_empty()
        && !key.starts_with('/')
        && key
            .split('/')
            .all(|seg| !seg.is_empty() && seg != ".." && seg != ".")
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
}

fn extension(r2_key: &str) -> Option<&str> {
    r2_key
        .rsplit('.')
        .next()
        .filter(|ext| !ext.is_empty() && ext.chars().all(|c| c.is_ascii_alphanumeric()))
}

pub fn cache_path(cache_dir: &Path, kind: AssetKind, asset: &AudioAsset) -> Option<PathBuf> {
    let id = safe_component(&asset.id)?;
    let ext = extension(&asset.r2_key)?;
    Some(
        cache_dir
            .join("packs")
            .join(kind.dir())
            .join(format!("{id}.{ext}")),
    )
}

pub fn cached(cache_dir: &Path, kind: AssetKind, asset: &AudioAsset) -> Option<PathBuf> {
    cache_path(cache_dir, kind, asset).filter(|path| path.exists())
}

/// List the asset ids already cached for a kind (offline-first inventory).
pub fn available(cache_dir: &Path, kind: AssetKind) -> Vec<String> {
    let dir = cache_dir.join("packs").join(kind.dir());
    let mut ids: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("part") {
                return None;
            }
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(String::from)
        })
        .collect();
    ids.sort();
    ids
}

/// Recognize common audio container magic bytes. Stands in as the integrity
/// "decode probe" — a real decode happens at playback.
pub fn looks_like_audio(bytes: &[u8]) -> bool {
    bytes.starts_with(b"OggS")
        || bytes.starts_with(b"ID3")
        || bytes.starts_with(b"fLaC")
        || bytes.starts_with(b"RIFF")
        || bytes.starts_with(&[0xFF, 0xF1])
        || bytes.starts_with(&[0xFF, 0xF9])
        || (bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] & 0xE0 == 0xE0)
        || (bytes.len() >= 8 && &bytes[4..8] == b"ftyp")
}

pub fn verify(bytes: &[u8], asset: &AudioAsset) -> Result<(), PackError> {
    if bytes.len() as u64 != asset.file_size_bytes {
        return Err(PackError::SizeMismatch {
            expected: asset.file_size_bytes,
            actual: bytes.len() as u64,
        });
    }
    if !looks_like_audio(bytes) {
        return Err(PackError::NotAudio);
    }
    Ok(())
}

pub fn fetch_manifest(fetcher: &dyn Fetcher, base_url: &str) -> Result<Manifest, PackError> {
    let bytes = fetcher.get(&format!("{base_url}/manifest.json"))?;
    serde_json::from_slice(&bytes).map_err(|e| PackError::Manifest(e.to_string()))
}

/// Download one asset: fetch the manifest, locate the asset, fetch and verify
/// its bytes in memory, then write to a `.part` file and atomically rename. A
/// failed verification never writes anything, so the cache is never poisoned.
pub fn download(
    fetcher: &dyn Fetcher,
    base_url: &str,
    cache_dir: &Path,
    kind: AssetKind,
    id: &str,
) -> Result<PathBuf, PackError> {
    let manifest = fetch_manifest(fetcher, base_url)?;
    let asset = manifest
        .assets_for(kind)
        .iter()
        .find(|asset| asset.id == id)
        .ok_or_else(|| PackError::UnknownAsset(id.to_string()))?;

    let dest = cache_path(cache_dir, kind, asset)
        .ok_or_else(|| PackError::UnsafeName(asset.id.clone()))?;
    if !safe_r2_key(&asset.r2_key) {
        return Err(PackError::UnsafeName(asset.r2_key.clone()));
    }
    let bytes = fetcher.get(&format!("{base_url}/{}", asset.r2_key))?;
    verify(&bytes, asset)?;

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let part = dest.with_extension("part");
    std::fs::write(&part, &bytes)?;
    std::fs::rename(&part, &dest)?;
    Ok(dest)
}

#[cfg(feature = "download")]
pub struct HttpFetcher {
    agent: ureq::Agent,
}

#[cfg(feature = "download")]
impl HttpFetcher {
    pub fn new() -> HttpFetcher {
        use std::time::Duration;
        HttpFetcher {
            agent: ureq::AgentBuilder::new()
                .https_only(true)
                .redirects(0)
                .timeout_connect(Duration::from_secs(15))
                .timeout_read(Duration::from_secs(60))
                .timeout_write(Duration::from_secs(30))
                .build(),
        }
    }
}

#[cfg(feature = "download")]
impl Default for HttpFetcher {
    fn default() -> HttpFetcher {
        HttpFetcher::new()
    }
}

#[cfg(feature = "download")]
impl Fetcher for HttpFetcher {
    fn get(&self, url: &str) -> Result<Vec<u8>, PackError> {
        use std::io::Read;
        if !url.starts_with("https://") {
            return Err(PackError::Http("refusing non-HTTPS URL".to_string()));
        }
        let response = self
            .agent
            .get(url)
            .call()
            .map_err(|e| PackError::Http(e.to_string()))?;
        let mut buffer = Vec::new();
        response
            .into_reader()
            .take(64 * 1024 * 1024)
            .read_to_end(&mut buffer)?;
        Ok(buffer)
    }
}
