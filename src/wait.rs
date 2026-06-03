//! Breathe while a shell command runs.
//!
//! `--until "<command>"` wraps the command as a child process, captures its
//! output to a temp log, and lets the breathing session run until it exits. The
//! session polls [`Waiter::poll`] each frame, so nothing blocks the orb.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const TAIL_BYTES: u64 = 4096;
const TAIL_LINES: usize = 8;

/// Distinguishes concurrent waiters' log files within one process.
static SEQ: AtomicU64 = AtomicU64::new(0);

/// How a wrapped command's wait ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaitStatus {
    /// The command exited on its own.
    Done { success: bool, code: Option<i32> },
    /// Ctrl-C while waiting — the command was killed.
    Cancelled,
    /// q/Esc while waiting — the command was left running.
    Detached,
}

/// What to print and notify once the wait is over.
#[derive(Debug, Clone)]
pub struct WaitReport {
    pub command: String,
    pub status: WaitStatus,
    pub elapsed: Duration,
    pub tail: String,
}

/// A wrapped command being waited on.
pub struct Waiter {
    child: Child,
    command: String,
    started: Instant,
    log: PathBuf,
}

#[cfg(not(windows))]
fn shell(command: &str) -> Command {
    let mut c = Command::new("sh");
    c.arg("-c").arg(command);
    c
}

#[cfg(windows)]
fn shell(command: &str) -> Command {
    let mut c = Command::new("cmd");
    c.arg("/C").arg(command);
    c
}

impl Waiter {
    /// Spawn the command via the shell (so pipes and `&&` work), with stdout and
    /// stderr redirected to a temp log so the buffers never block the child.
    pub fn spawn(command: &str) -> std::io::Result<Waiter> {
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let log =
            std::env::temp_dir().join(format!("meditate-wait-{}-{seq}.log", std::process::id()));
        let out = File::create(&log)?;
        let err = out.try_clone()?;
        let child = shell(command)
            .stdin(Stdio::null())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()?;
        Ok(Waiter {
            child,
            command: command.to_string(),
            started: Instant::now(),
            log,
        })
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    /// `Some(report)` once the command has exited; `None` while it is still
    /// running. Non-blocking.
    pub fn poll(&mut self) -> Option<WaitReport> {
        match self.child.try_wait() {
            Ok(Some(status)) => Some(self.report(WaitStatus::Done {
                success: status.success(),
                code: status.code(),
            })),
            Ok(None) => None,
            Err(_) => Some(self.report(WaitStatus::Done {
                success: false,
                code: None,
            })),
        }
    }

    /// Kill the command (Ctrl-C while waiting).
    pub fn cancel(mut self) -> WaitReport {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.report(WaitStatus::Cancelled)
    }

    /// Leave the command running (q/Esc while waiting).
    pub fn detach(self) -> WaitReport {
        self.report(WaitStatus::Detached)
    }

    fn report(&self, status: WaitStatus) -> WaitReport {
        WaitReport {
            command: self.command.clone(),
            status,
            elapsed: self.started.elapsed(),
            tail: tail_of(&self.log),
        }
    }
}

impl Drop for Waiter {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.log);
    }
}

/// The last few lines of a log file, read losslessly from its tail.
fn tail_of(path: &Path) -> String {
    let Ok(mut file) = File::open(path) else {
        return String::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if file
        .seek(SeekFrom::Start(len.saturating_sub(TAIL_BYTES)))
        .is_err()
    {
        return String::new();
    }
    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(TAIL_LINES)..]
        .join("\n")
        .trim()
        .to_string()
}

/// Human duration like `2m 13s` or `9s`.
pub fn human_duration(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    if secs >= 60 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

/// The desktop-notification body for a finished command, or `None` when there's
/// nothing worth pinging about (cancelled or detached — the user is right here).
pub fn notify_message(report: &WaitReport) -> Option<String> {
    match report.status {
        WaitStatus::Done { success: true, .. } => Some(format!("✓ done: {}", report.command)),
        WaitStatus::Done { success: false, .. } => Some(format!("✗ failed: {}", report.command)),
        WaitStatus::Cancelled | WaitStatus::Detached => None,
    }
}

/// Print the post-session report line (and, on failure, the captured tail).
pub fn print_report(report: &WaitReport) {
    let elapsed = human_duration(report.elapsed);
    match &report.status {
        WaitStatus::Done { success: true, .. } => {
            println!("  ✓ {} — {elapsed}", report.command);
        }
        WaitStatus::Done {
            success: false,
            code,
        } => {
            let code = code.map_or_else(|| "killed".to_string(), |c| format!("exited {c}"));
            println!("  ✗ {} — {code} ({elapsed})", report.command);
            for line in report.tail.lines() {
                println!("    {line}");
            }
        }
        WaitStatus::Cancelled => println!("  ■ {} — cancelled after {elapsed}", report.command),
        WaitStatus::Detached => println!("  → {} — left running", report.command),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn run_to_completion(command: &str) -> WaitReport {
        let mut waiter = Waiter::spawn(command).unwrap();
        loop {
            if let Some(report) = waiter.poll() {
                return report;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(unix)]
    #[test]
    fn poll_reports_success() {
        let report = run_to_completion("exit 0");
        assert_eq!(
            report.status,
            WaitStatus::Done {
                success: true,
                code: Some(0)
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn poll_reports_failure_with_code_and_tail() {
        let report = run_to_completion("echo boom 1>&2; exit 3");
        assert_eq!(
            report.status,
            WaitStatus::Done {
                success: false,
                code: Some(3)
            }
        );
        assert!(report.tail.contains("boom"), "tail was {:?}", report.tail);
    }

    #[test]
    fn human_duration_switches_at_a_minute() {
        assert_eq!(human_duration(Duration::from_secs(9)), "9s");
        assert_eq!(human_duration(Duration::from_secs(133)), "2m 13s");
    }

    #[test]
    fn notify_message_only_for_finished_commands() {
        let done_ok = WaitReport {
            command: "cargo build".into(),
            status: WaitStatus::Done {
                success: true,
                code: Some(0),
            },
            elapsed: Duration::from_secs(1),
            tail: String::new(),
        };
        assert_eq!(
            notify_message(&done_ok).as_deref(),
            Some("✓ done: cargo build")
        );

        let detached = WaitReport {
            status: WaitStatus::Detached,
            ..done_ok
        };
        assert_eq!(notify_message(&detached), None);
    }
}
