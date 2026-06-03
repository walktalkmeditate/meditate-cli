pub mod soundscape;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// CDN base for soundscapes + bells. The audio manifest lives at
/// `{AUDIO_BASE_URL}/manifest.json`; each file at `{AUDIO_BASE_URL}/{type}/{id}.aac`.
/// Mirrors `Config.Audio` in pilgrim-ios.
pub const AUDIO_BASE_URL: &str = "https://cdn.pilgrimapp.org/audio";

/// CDN base for voice guides. The voice manifest lives at
/// `{VOICE_BASE_URL}/manifest.json`; each prompt at `{VOICE_BASE_URL}/{packId}/{promptId}.aac`.
/// Mirrors `Config.VoiceGuide` in pilgrim-ios.
pub const VOICE_BASE_URL: &str = "https://cdn.pilgrimapp.org/voiceguide";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetKind {
    Soundscape,
    Voice,
    Bell,
}

impl AssetKind {
    /// The cache subdirectory name.
    pub fn dir(self) -> &'static str {
        match self {
            AssetKind::Soundscape => "soundscapes",
            AssetKind::Voice => "voices",
            AssetKind::Bell => "bells",
        }
    }

    /// The `type` discriminator used in the audio manifest. Voices live in a
    /// separate manifest and have no audio-manifest type.
    pub fn audio_type(self) -> Option<&'static str> {
        match self {
            AssetKind::Soundscape => Some("soundscape"),
            AssetKind::Bell => Some("bell"),
            AssetKind::Voice => None,
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

/// One soundscape or bell. Field names mirror pilgrim-ios `AudioAsset`. The
/// download URL is built from `type` + `id`, not `r2Key`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AudioAsset {
    pub id: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "displayName", default)]
    pub display_name: String,
    #[serde(rename = "durationSec", default)]
    pub duration_sec: f64,
    #[serde(rename = "r2Key", default)]
    pub r2_key: String,
    #[serde(rename = "fileSizeBytes", default)]
    pub file_size_bytes: u64,
    #[serde(rename = "usageTags", default, deserialize_with = "null_as_default")]
    pub usage_tags: Vec<String>,
}

/// The audio manifest: a flat asset list discriminated by `type`. Mirrors
/// pilgrim-ios `AudioManifest`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AudioManifest {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub assets: Vec<AudioAsset>,
}

impl AudioManifest {
    /// Assets matching a kind's `type`. Returns empty for `Voice` (a different
    /// manifest).
    pub fn assets_for(&self, kind: AssetKind) -> Vec<&AudioAsset> {
        match kind.audio_type() {
            Some(want) => self.assets.iter().filter(|a| a.kind == want).collect(),
            None => Vec::new(),
        }
    }
}

/// A single meditation voice prompt. Mirrors the meditation subset of
/// pilgrim-ios `VoiceGuidePrompt`; walk prompts are never deserialized here.
/// Serializable so a downloaded pack can persist its prompt list locally.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MeditationPrompt {
    pub id: String,
    #[serde(default)]
    pub seq: u32,
    #[serde(rename = "durationSec", default)]
    pub duration_sec: f64,
    #[serde(rename = "fileSizeBytes", default)]
    pub file_size_bytes: u64,
    #[serde(rename = "r2Key", default)]
    pub r2_key: String,
    #[serde(default)]
    pub phase: Option<String>,
}

/// A voice guide pack. Only the fields the CLI needs are modeled — the rest of
/// pilgrim-ios `VoiceGuidePack` (walk prompts, scheduling, theme, …) is ignored.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct VoicePack {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub tagline: String,
    #[serde(
        rename = "meditationPrompts",
        default,
        deserialize_with = "null_as_default"
    )]
    pub meditation_prompts: Vec<MeditationPrompt>,
}

/// The voice manifest. Mirrors pilgrim-ios `VoiceGuideManifest`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct VoiceManifest {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub packs: Vec<VoicePack>,
}

/// Deserialize a value that may be JSON `null` into `T::default()` rather than
/// erroring — the manifest marks optional arrays as nullable.
fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
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

/// Validate a single path/URL component against an allowlist (no separators, no
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

/// Where a soundscape or bell is cached: `packs/{dir}/{id}.aac`.
pub fn cache_path(cache_dir: &Path, kind: AssetKind, id: &str) -> Option<PathBuf> {
    let id = safe_component(id)?;
    Some(
        cache_dir
            .join("packs")
            .join(kind.dir())
            .join(format!("{id}.aac")),
    )
}

/// The directory holding a downloaded voice pack: `packs/voices/{packId}`.
pub fn voice_pack_dir(cache_dir: &Path, pack_id: &str) -> Option<PathBuf> {
    let pack_id = safe_component(pack_id)?;
    Some(cache_dir.join("packs").join("voices").join(pack_id))
}

/// List cached files for a kind as (id, path) pairs, sorted by id. Skips the
/// `.part` quarantine files. For voices this lists pack directories.
pub fn cached_files(cache_dir: &Path, kind: AssetKind) -> Vec<(String, PathBuf)> {
    let dir = cache_dir.join("packs").join(kind.dir());
    let mut files: Vec<(String, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("part") {
                return None;
            }
            let id = path.file_stem()?.to_str()?.to_string();
            Some((id, path))
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

/// List the asset ids already cached for a kind (offline-first inventory).
pub fn available(cache_dir: &Path, kind: AssetKind) -> Vec<String> {
    cached_files(cache_dir, kind)
        .into_iter()
        .map(|(id, _)| id)
        .collect()
}

/// Size + magic-byte integrity gate. A zero `expected` size means the manifest
/// omitted it, so only the magic-byte check applies.
pub fn verify(bytes: &[u8], expected_size: u64) -> Result<(), PackError> {
    if expected_size != 0 && bytes.len() as u64 != expected_size {
        return Err(PackError::SizeMismatch {
            expected: expected_size,
            actual: bytes.len() as u64,
        });
    }
    if !looks_like_audio(bytes) {
        return Err(PackError::NotAudio);
    }
    Ok(())
}

/// Verify bytes, then write them through a `.part` quarantine and atomically
/// rename into place. A failed verification never writes, so the cache is never
/// poisoned by a partial or corrupt file.
fn write_verified(dest: &Path, bytes: &[u8], expected_size: u64) -> Result<(), PackError> {
    verify(bytes, expected_size)?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let part = dest.with_extension("part");
    std::fs::write(&part, bytes)?;
    std::fs::rename(&part, dest)?;
    Ok(())
}

pub fn fetch_audio_manifest(fetcher: &dyn Fetcher) -> Result<AudioManifest, PackError> {
    let bytes = fetcher.get(&format!("{AUDIO_BASE_URL}/manifest.json"))?;
    serde_json::from_slice(&bytes).map_err(|e| PackError::Manifest(e.to_string()))
}

pub fn fetch_voice_manifest(fetcher: &dyn Fetcher) -> Result<VoiceManifest, PackError> {
    let bytes = fetcher.get(&format!("{VOICE_BASE_URL}/manifest.json"))?;
    serde_json::from_slice(&bytes).map_err(|e| PackError::Manifest(e.to_string()))
}

/// Whether a download fetched bytes or found the asset already cached.
#[derive(Debug)]
pub enum DownloadOutcome {
    Downloaded(PathBuf),
    AlreadyCached(PathBuf),
}

impl DownloadOutcome {
    pub fn path(&self) -> &Path {
        match self {
            DownloadOutcome::Downloaded(p) | DownloadOutcome::AlreadyCached(p) => p,
        }
    }
}

/// Download a known soundscape or bell, skipping the fetch when it is already
/// cached. The caller supplies the asset it already read from the manifest, so
/// no second manifest fetch happens.
pub fn download_audio_asset(
    fetcher: &dyn Fetcher,
    cache_dir: &Path,
    kind: AssetKind,
    asset: &AudioAsset,
) -> Result<DownloadOutcome, PackError> {
    let dest = cache_path(cache_dir, kind, &asset.id)
        .ok_or_else(|| PackError::UnsafeName(asset.id.clone()))?;
    if dest.exists() {
        return Ok(DownloadOutcome::AlreadyCached(dest));
    }
    let asset_type = kind
        .audio_type()
        .ok_or_else(|| PackError::UnknownAsset(asset.id.clone()))?;
    let safe_id =
        safe_component(&asset.id).ok_or_else(|| PackError::UnsafeName(asset.id.clone()))?;

    let url = format!("{AUDIO_BASE_URL}/{asset_type}/{safe_id}.aac");
    let bytes = fetcher.get(&url)?;
    write_verified(&dest, &bytes, asset.file_size_bytes)?;
    Ok(DownloadOutcome::Downloaded(dest))
}

/// Download one soundscape or bell by id: locate it in the audio manifest, then
/// fetch (or skip if already cached).
pub fn download_audio(
    fetcher: &dyn Fetcher,
    cache_dir: &Path,
    kind: AssetKind,
    id: &str,
) -> Result<DownloadOutcome, PackError> {
    let manifest = fetch_audio_manifest(fetcher)?;
    let asset = manifest
        .assets_for(kind)
        .into_iter()
        .find(|asset| asset.id == id)
        .ok_or_else(|| PackError::UnknownAsset(id.to_string()))?;
    download_audio_asset(fetcher, cache_dir, kind, asset)
}

/// Download a voice pack the caller already read from the manifest: fetch each
/// meditation prompt that is not already cached to
/// `packs/voices/{packId}/{promptId}.aac`, then persist the prompt list as
/// `meditation.json` for offline scheduling. `AlreadyCached` means every prompt
/// (and the manifest) was already present.
pub fn download_voice_pack_from(
    fetcher: &dyn Fetcher,
    cache_dir: &Path,
    pack: &VoicePack,
) -> Result<DownloadOutcome, PackError> {
    if pack.meditation_prompts.is_empty() {
        return Err(PackError::UnknownAsset(pack.id.clone()));
    }
    let safe_pack =
        safe_component(&pack.id).ok_or_else(|| PackError::UnsafeName(pack.id.clone()))?;
    let dir = voice_pack_dir(cache_dir, &pack.id)
        .ok_or_else(|| PackError::UnsafeName(pack.id.clone()))?;

    let mut fetched_any = false;
    for prompt in &pack.meditation_prompts {
        let safe_id =
            safe_component(&prompt.id).ok_or_else(|| PackError::UnsafeName(prompt.id.clone()))?;
        let dest = dir.join(format!("{safe_id}.aac"));
        if dest.exists() {
            continue;
        }
        let url = format!("{VOICE_BASE_URL}/{safe_pack}/{safe_id}.aac");
        let bytes = fetcher.get(&url)?;
        write_verified(&dest, &bytes, prompt.file_size_bytes)?;
        fetched_any = true;
    }

    let meta_path = dir.join("meditation.json");
    if fetched_any || !meta_path.exists() {
        std::fs::create_dir_all(&dir)?;
        let meta = serde_json::to_vec_pretty(&pack.meditation_prompts)
            .map_err(|e| PackError::Manifest(e.to_string()))?;
        std::fs::write(&meta_path, meta)?;
    }

    Ok(if fetched_any {
        DownloadOutcome::Downloaded(dir)
    } else {
        DownloadOutcome::AlreadyCached(dir)
    })
}

/// Download a voice pack by id (fetches the voice manifest to find it).
pub fn download_voice_pack(
    fetcher: &dyn Fetcher,
    cache_dir: &Path,
    pack_id: &str,
) -> Result<DownloadOutcome, PackError> {
    let manifest = fetch_voice_manifest(fetcher)?;
    let pack = manifest
        .packs
        .iter()
        .find(|pack| pack.id == pack_id)
        .ok_or_else(|| PackError::UnknownAsset(pack_id.to_string()))?;
    download_voice_pack_from(fetcher, cache_dir, pack)
}

/// Load the meditation prompts a downloaded voice pack persisted locally.
pub fn load_voice_prompts(pack_dir: &Path) -> Option<Vec<MeditationPrompt>> {
    let bytes = std::fs::read(pack_dir.join("meditation.json")).ok()?;
    serde_json::from_slice(&bytes).ok()
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
