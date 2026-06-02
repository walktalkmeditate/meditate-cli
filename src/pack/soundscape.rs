use std::path::Path;

/// Decode a cached soundscape file into mono f32 samples for the mixer.
///
/// The real Pilgrim assets are ADTS-AAC and OGG-Vorbis. Wiring a decoder (e.g.
/// `symphonia` behind the `audio` feature) is the remaining plug-in; until then
/// this returns `None`, and the session falls back to the missing-pack hint —
/// so an absent decoder never breaks the breathing screen.
pub fn load_samples(_path: &Path) -> Option<Vec<f32>> {
    None
}
