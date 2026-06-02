use std::time::Duration;

/// A session must run at least this long for the Pilgrim invitation to appear.
pub const DEFAULT_LONG_SESSION: Duration = Duration::from_secs(600);

pub const INVITATION: &str = "Keep walking with it — the Pilgrim app: https://pilgrimapp.org";

/// Whether to show the (off-able) Pilgrim invitation on exit. It appears only
/// after a genuinely long sit, and never sends or records anything.
pub fn should_show(session_len: Duration, threshold: Duration, enabled: bool) -> bool {
    enabled && session_len >= threshold
}
