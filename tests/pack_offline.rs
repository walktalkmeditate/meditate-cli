use meditate::pack::{
    available, download_audio, download_voice_pack, fetch_audio_manifest, fetch_voice_manifest,
    load_voice_prompts, looks_like_audio, safe_component, verify, AssetKind, Fetcher, PackError,
    AUDIO_BASE_URL, VOICE_BASE_URL,
};
use std::cell::RefCell;
use std::collections::HashMap;

/// In-memory fetcher that serves canned bytes and records every requested URL,
/// so download orchestration (and what it does *not* fetch) is testable offline.
struct FakeFetcher {
    responses: HashMap<String, Vec<u8>>,
    requested: RefCell<Vec<String>>,
}

impl FakeFetcher {
    fn new() -> FakeFetcher {
        FakeFetcher {
            responses: HashMap::new(),
            requested: RefCell::new(Vec::new()),
        }
    }

    fn with(mut self, url: String, bytes: Vec<u8>) -> FakeFetcher {
        self.responses.insert(url, bytes);
        self
    }

    fn requested(&self, url: &str) -> bool {
        self.requested.borrow().iter().any(|u| u == url)
    }
}

impl Fetcher for FakeFetcher {
    fn get(&self, url: &str) -> Result<Vec<u8>, PackError> {
        self.requested.borrow_mut().push(url.to_string());
        self.responses
            .get(url)
            .cloned()
            .ok_or_else(|| PackError::Http(format!("404 {url}")))
    }
}

/// Minimal ADTS-AAC header padded to `len`, accepted by `looks_like_audio`.
fn aac(len: usize) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xF1];
    bytes.resize(len.max(2), 0);
    bytes
}

const AUDIO_MANIFEST: &str = r#"{
  "version": "1",
  "assets": [
    {"id":"forest","type":"soundscape","name":"forest","displayName":"Forest",
     "durationSec":120,"fileSizeBytes":8,"r2Key":"soundscape/forest.aac","usageTags":[]},
    {"id":"temple","type":"bell","name":"temple","displayName":"Temple",
     "durationSec":4,"fileSizeBytes":8,"r2Key":"bell/temple.aac","usageTags":["intro"]}
  ]
}"#;

// A pack carrying BOTH walk prompts and meditation prompts — the CLI must model
// and fetch only the meditation set.
const VOICE_MANIFEST: &str = r#"{
  "version": "1",
  "packs": [
    {"id":"gentle","name":"Gentle","tagline":"soft","type":"guide","walkTypes":["forest"],
     "scheduling":{"densityMinSec":1,"densityMaxSec":2,"minSpacingSec":1,"initialDelaySec":1,"walkEndBufferSec":1},
     "totalDurationSec":10,"totalSizeBytes":16,
     "prompts":[{"id":"walk-1","seq":1,"durationSec":3,"fileSizeBytes":8,"r2Key":"voiceguide/gentle/walk-1.aac"}],
     "meditationScheduling":{"densityMinSec":1,"densityMaxSec":2,"minSpacingSec":1,"initialDelaySec":1,"walkEndBufferSec":1},
     "meditationPrompts":[
       {"id":"med-settle","seq":1,"durationSec":5,"fileSizeBytes":8,"r2Key":"voiceguide/gentle/med-settle.aac","phase":"settling"},
       {"id":"med-close","seq":2,"durationSec":5,"fileSizeBytes":8,"r2Key":"voiceguide/gentle/med-close.aac","phase":"closing"}
     ]},
    {"id":"walkonly","name":"Walk Only","tagline":"t","type":"guide","walkTypes":[],
     "scheduling":{"densityMinSec":1,"densityMaxSec":2,"minSpacingSec":1,"initialDelaySec":1,"walkEndBufferSec":1},
     "totalDurationSec":3,"totalSizeBytes":8,
     "prompts":[{"id":"w1","seq":1,"durationSec":3,"fileSizeBytes":8,"r2Key":"x"}],
     "meditationPrompts":null}
  ]
}"#;

fn audio_fetcher(asset_body: Vec<u8>) -> FakeFetcher {
    FakeFetcher::new()
        .with(
            format!("{AUDIO_BASE_URL}/manifest.json"),
            AUDIO_MANIFEST.as_bytes().to_vec(),
        )
        .with(
            format!("{AUDIO_BASE_URL}/soundscape/forest.aac"),
            asset_body,
        )
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
    assert!(!looks_like_audio(b"notaudio"));
}

#[test]
fn verify_checks_size_and_format() {
    assert!(verify(&aac(8), 8).is_ok());
    assert!(matches!(
        verify(&aac(4), 8),
        Err(PackError::SizeMismatch {
            expected: 8,
            actual: 4
        })
    ));
    assert!(matches!(verify(b"notaudio", 8), Err(PackError::NotAudio)));
    // A zero expected size means the manifest omitted it — only magic is checked.
    assert!(verify(&aac(99), 0).is_ok());
}

#[test]
fn audio_manifest_parses_flat_assets_filtered_by_type() {
    let manifest = fetch_audio_manifest(&audio_fetcher(aac(8))).unwrap();
    assert_eq!(manifest.version, "1");
    let soundscapes = manifest.assets_for(AssetKind::Soundscape);
    assert_eq!(soundscapes.len(), 1);
    assert_eq!(soundscapes[0].id, "forest");
    let bells = manifest.assets_for(AssetKind::Bell);
    assert_eq!(bells.len(), 1);
    assert_eq!(bells[0].display_name, "Temple");
    assert!(manifest.assets_for(AssetKind::Voice).is_empty());
}

#[test]
fn download_audio_builds_type_id_url_and_caches_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let fetcher = audio_fetcher(aac(8));
    let path = download_audio(&fetcher, dir.path(), AssetKind::Soundscape, "forest").unwrap();

    assert!(path.ends_with("packs/soundscapes/forest.aac"));
    assert!(path.exists());
    assert_eq!(std::fs::read(&path).unwrap(), aac(8));
    assert!(!path.with_extension("part").exists());
    assert!(fetcher.requested(&format!("{AUDIO_BASE_URL}/soundscape/forest.aac")));
    assert!(available(dir.path(), AssetKind::Soundscape).contains(&"forest".to_string()));
}

#[test]
fn a_truncated_download_never_reaches_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let result = download_audio(
        &audio_fetcher(aac(4)),
        dir.path(),
        AssetKind::Soundscape,
        "forest",
    );
    assert!(matches!(result, Err(PackError::SizeMismatch { .. })));
    assert!(available(dir.path(), AssetKind::Soundscape).is_empty());
}

#[test]
fn unknown_asset_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let result = download_audio(
        &audio_fetcher(aac(8)),
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
fn voice_manifest_models_only_meditation_prompts() {
    let fetcher = FakeFetcher::new().with(
        format!("{VOICE_BASE_URL}/manifest.json"),
        VOICE_MANIFEST.as_bytes().to_vec(),
    );
    let manifest = fetch_voice_manifest(&fetcher).unwrap();
    let gentle = &manifest.packs[0];
    // Walk prompts (`prompts`) are never deserialized; only meditation prompts.
    assert_eq!(gentle.meditation_prompts.len(), 2);
    assert_eq!(gentle.meditation_prompts[0].id, "med-settle");
    assert_eq!(
        gentle.meditation_prompts[1].phase.as_deref(),
        Some("closing")
    );
    // A pack with null meditationPrompts is tolerated and empty.
    assert!(manifest.packs[1].meditation_prompts.is_empty());
}

#[test]
fn download_voice_pack_fetches_only_meditation_audio_never_walk() {
    let dir = tempfile::tempdir().unwrap();
    let fetcher = FakeFetcher::new()
        .with(
            format!("{VOICE_BASE_URL}/manifest.json"),
            VOICE_MANIFEST.as_bytes().to_vec(),
        )
        .with(format!("{VOICE_BASE_URL}/gentle/med-settle.aac"), aac(8))
        .with(format!("{VOICE_BASE_URL}/gentle/med-close.aac"), aac(8));

    let pack_dir = download_voice_pack(&fetcher, dir.path(), "gentle").unwrap();

    // Both meditation prompts fetched and cached.
    assert!(fetcher.requested(&format!("{VOICE_BASE_URL}/gentle/med-settle.aac")));
    assert!(fetcher.requested(&format!("{VOICE_BASE_URL}/gentle/med-close.aac")));
    assert!(pack_dir.join("med-settle.aac").exists());
    assert!(pack_dir.join("med-close.aac").exists());

    // The walk prompt is NEVER requested — no URL containing "walk-1".
    assert!(
        !fetcher
            .requested
            .borrow()
            .iter()
            .any(|u| u.contains("walk-1")),
        "walk audio must never be fetched"
    );

    // The prompt list is persisted for offline scheduling.
    let prompts = load_voice_prompts(&pack_dir).expect("meditation.json persisted");
    assert_eq!(prompts.len(), 2);
}

#[test]
fn download_voice_pack_without_meditation_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let fetcher = FakeFetcher::new().with(
        format!("{VOICE_BASE_URL}/manifest.json"),
        VOICE_MANIFEST.as_bytes().to_vec(),
    );
    let result = download_voice_pack(&fetcher, dir.path(), "walkonly");
    assert!(matches!(result, Err(PackError::UnknownAsset(_))));
}
