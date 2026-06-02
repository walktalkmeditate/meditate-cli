use meditate::pack::{
    available, cache_path, cached, download, fetch_manifest, looks_like_audio, safe_component,
    verify, AssetKind, Fetcher, PackError,
};
use std::collections::HashMap;

const BASE: &str = "https://cdn.example/audio";

struct FakeFetcher {
    responses: HashMap<String, Vec<u8>>,
}

impl Fetcher for FakeFetcher {
    fn get(&self, url: &str) -> Result<Vec<u8>, PackError> {
        self.responses
            .get(url)
            .cloned()
            .ok_or_else(|| PackError::Http(format!("404 {url}")))
    }
}

const MANIFEST: &str = r#"{"soundscapes":[{"id":"forest","type":"soundscape","displayName":"Forest","durationSec":120,"fileSizeBytes":8,"r2Key":"soundscape/forest.ogg"}]}"#;

const VOICES_MANIFEST: &str = r#"{"voices":[{"id":"gentle","type":"voice","fileSizeBytes":4,"r2Key":"voice/gentle.aac","walkPrompts":[{"id":"w1","r2Key":"x.aac","phase":"walk"}],"meditationPrompts":[{"id":"m1","r2Key":"voice/m1.aac","phase":"settling"}]}]}"#;

fn ogg_bytes() -> Vec<u8> {
    b"OggS\x00\x00\x00\x00".to_vec()
}

fn fetcher(asset_body: Vec<u8>) -> FakeFetcher {
    let mut responses = HashMap::new();
    responses.insert(
        format!("{BASE}/manifest.json"),
        MANIFEST.as_bytes().to_vec(),
    );
    responses.insert(format!("{BASE}/soundscape/forest.ogg"), asset_body);
    FakeFetcher { responses }
}

#[test]
fn manifest_parses_the_expected_schema() {
    let manifest = fetch_manifest(&fetcher(ogg_bytes()), BASE).unwrap();
    assert_eq!(manifest.soundscapes.len(), 1);
    let asset = &manifest.soundscapes[0];
    assert_eq!(asset.id, "forest");
    assert_eq!(asset.file_size_bytes, 8);
    assert_eq!(asset.r2_key, "soundscape/forest.ogg");
}

#[test]
fn safe_component_blocks_traversal_and_separators() {
    assert_eq!(safe_component("forest"), Some("forest".to_string()));
    assert_eq!(safe_component("a.b_c-d"), Some("a.b_c-d".to_string()));
    assert_eq!(safe_component("../etc/passwd"), None);
    assert_eq!(safe_component("a/b"), None);
    assert_eq!(safe_component(""), None);
}

#[test]
fn audio_magic_is_recognized() {
    assert!(looks_like_audio(b"OggS....."));
    assert!(looks_like_audio(&[0xFF, 0xF1, 0x00]));
    assert!(!looks_like_audio(b"not audio"));
}

#[test]
fn verify_checks_size_and_format() {
    let manifest = fetch_manifest(&fetcher(ogg_bytes()), BASE).unwrap();
    let asset = &manifest.soundscapes[0];
    assert!(verify(&ogg_bytes(), asset).is_ok());
    assert!(matches!(
        verify(b"OggS", asset),
        Err(PackError::SizeMismatch { .. })
    ));
    assert!(matches!(
        verify(b"notaudio", asset),
        Err(PackError::NotAudio)
    ));
}

#[test]
fn cache_path_lands_under_the_kind_directory() {
    let manifest = fetch_manifest(&fetcher(ogg_bytes()), BASE).unwrap();
    let asset = &manifest.soundscapes[0];
    let path = cache_path(std::path::Path::new("/cache"), AssetKind::Soundscape, asset).unwrap();
    assert!(path.ends_with("packs/soundscapes/forest.ogg"));
}

#[test]
fn download_verifies_then_caches_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let path = download(
        &fetcher(ogg_bytes()),
        BASE,
        dir.path(),
        AssetKind::Soundscape,
        "forest",
    )
    .unwrap();

    assert!(path.exists());
    assert_eq!(std::fs::read(&path).unwrap(), ogg_bytes());
    assert!(!path.with_extension("part").exists());
    assert!(available(dir.path(), AssetKind::Soundscape).contains(&"forest".to_string()));
}

#[test]
fn a_truncated_download_never_reaches_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let result = download(
        &fetcher(b"OggS".to_vec()),
        BASE,
        dir.path(),
        AssetKind::Soundscape,
        "forest",
    );
    assert!(matches!(result, Err(PackError::SizeMismatch { .. })));

    let manifest = fetch_manifest(&fetcher(ogg_bytes()), BASE).unwrap();
    assert!(cached(dir.path(), AssetKind::Soundscape, &manifest.soundscapes[0]).is_none());
}

#[test]
fn unknown_asset_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let result = download(
        &fetcher(ogg_bytes()),
        BASE,
        dir.path(),
        AssetKind::Soundscape,
        "ocean",
    );
    assert!(matches!(result, Err(PackError::UnknownAsset(_))));
}

#[test]
fn fresh_cache_is_empty_offline() {
    let dir = tempfile::tempdir().unwrap();
    assert!(available(dir.path(), AssetKind::Soundscape).is_empty());
}

#[test]
fn voice_manifest_keeps_only_meditation_prompts() {
    let mut responses = HashMap::new();
    responses.insert(
        format!("{BASE}/manifest.json"),
        VOICES_MANIFEST.as_bytes().to_vec(),
    );
    let manifest = fetch_manifest(&FakeFetcher { responses }, BASE).unwrap();
    let voice = &manifest.voices[0];
    assert_eq!(voice.meditation_prompts.len(), 1);
    assert_eq!(voice.meditation_prompts[0].id, "m1");
}
