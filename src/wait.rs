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
    use std::os::unix::process::CommandExt;
    let mut c = Command::new("sh");
    // Lead its own process group so cancel can signal the whole command tree.
    c.arg("-c").arg(command).process_group(0);
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
            Err(err) => {
                // A poll failure is not the command failing — surface it rather
                // than silently reporting a false failure with no detail.
                let mut report = self.report(WaitStatus::Done {
                    success: false,
                    code: None,
                });
                report.tail = format!("could not poll the command: {err}\n{}", report.tail);
                Some(report)
            }
        }
    }

    /// Kill the command (Ctrl-C while waiting), taking down its whole process
    /// group so the command's children die too, not just the launched shell.
    pub fn cancel(mut self) -> WaitReport {
        self.kill_tree();
        let _ = self.child.wait();
        self.report(WaitStatus::Cancelled)
    }

    #[cfg(unix)]
    fn kill_tree(&mut self) {
        // The child leads its own process group (see `shell`); a negative pid
        // signals the whole group.
        unsafe { libc::kill(-(self.child.id() as i32), libc::SIGKILL) };
    }

    #[cfg(windows)]
    fn kill_tree(&mut self) {
        let _ = self.child.kill();
    }

    /// Leave the command running (q/Esc while waiting).
    pub fn detach(self) -> WaitReport {
        let report = self.report(WaitStatus::Detached);
        // Don't reap the child or delete its log out from under it — it keeps
        // writing as it runs on.
        std::mem::forget(self);
        report
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
        return "(could not read the captured output)".to_string();
    }
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    let tail = lines[lines.len().saturating_sub(TAIL_LINES)..].join("\n");
    strip_controls(tail.trim(), true)
}

/// Drop terminal control bytes (ESC, BEL, CSI/OSC introducers, DEL, C1) so
/// captured program output and the command string can't drive the real terminal
/// when printed or sent in an escape. `keep_breaks` preserves newlines and tabs
/// for multi-line output; pass false for single-line escape bodies.
fn strip_controls(text: &str, keep_breaks: bool) -> String {
    text.chars()
        .filter(|&c| (keep_breaks && (c == '\n' || c == '\t')) || !c.is_control())
        .collect()
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
    let command = strip_controls(&report.command, false);
    match report.status {
        WaitStatus::Done { success: true, .. } => Some(format!("✓ done: {command}")),
        WaitStatus::Done { success: false, .. } => Some(format!("✗ failed: {command}")),
        WaitStatus::Cancelled | WaitStatus::Detached => None,
    }
}

/// Print the post-session report line (and, on failure, the captured tail).
pub fn print_report(report: &WaitReport) {
    let elapsed = human_duration(report.elapsed);
    let command = strip_controls(&report.command, false);
    match &report.status {
        WaitStatus::Done { success: true, .. } => {
            println!("  ✓ {command} — {elapsed}");
        }
        WaitStatus::Done {
            success: false,
            code,
        } => {
            let code = code.map_or_else(|| "killed".to_string(), |c| format!("exited {c}"));
            println!("  ✗ {command} — {code} ({elapsed})");
            for line in report.tail.lines() {
                println!("    {line}");
            }
        }
        WaitStatus::Cancelled => println!("  ■ {command} — cancelled after {elapsed}"),
        WaitStatus::Detached => println!("  → {command} — left running"),
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

    #[cfg(unix)]
    #[test]
    fn cancel_kills_the_command() {
        let mut waiter = Waiter::spawn("sleep 30").unwrap();
        assert!(waiter.poll().is_none()); // still running
        let report = waiter.cancel();
        assert_eq!(report.status, WaitStatus::Cancelled);
    }

    #[cfg(unix)]
    #[test]
    fn cancel_kills_the_whole_process_group() {
        // sh backgrounds a long-lived grandchild, logs its pid, then waits on it.
        let waiter = Waiter::spawn("sleep 30 & echo $!; wait").unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let grandchild: i32 = waiter
            .report(WaitStatus::Cancelled)
            .tail
            .lines()
            .last()
            .and_then(|line| line.trim().parse().ok())
            .expect("grandchild pid in the log");

        waiter.cancel();
        std::thread::sleep(Duration::from_millis(150));

        // kill(pid, 0) probes existence: 0 = alive, error/ESRCH = gone.
        let alive = unsafe { libc::kill(grandchild, 0) } == 0;
        assert!(!alive, "grandchild {grandchild} survived cancel");
    }

    #[cfg(unix)]
    #[test]
    fn detach_leaves_the_command_running() {
        let waiter = Waiter::spawn("sleep 0.2").unwrap();
        let report = waiter.detach();
        assert_eq!(report.status, WaitStatus::Detached);
    }

    #[test]
    fn notify_message_covers_every_status() {
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

        let failed = WaitReport {
            status: WaitStatus::Done {
                success: false,
                code: Some(1),
            },
            ..done_ok.clone()
        };
        assert_eq!(
            notify_message(&failed).as_deref(),
            Some("✗ failed: cargo build")
        );

        let detached = WaitReport {
            status: WaitStatus::Detached,
            ..done_ok.clone()
        };
        assert_eq!(notify_message(&detached), None);

        let cancelled = WaitReport {
            status: WaitStatus::Cancelled,
            ..done_ok
        };
        assert_eq!(notify_message(&cancelled), None);
    }

    #[test]
    fn control_bytes_are_stripped_from_notifications_and_tails() {
        let nasty = WaitReport {
            command: "echo \x1b]0;pwned\x07 hi".into(),
            status: WaitStatus::Done {
                success: false,
                code: Some(1),
            },
            elapsed: Duration::from_secs(1),
            tail: String::new(),
        };
        let message = notify_message(&nasty).unwrap();
        assert!(!message.contains('\x1b'), "ESC leaked: {message:?}");
        assert!(!message.contains('\x07'), "BEL leaked: {message:?}");

        // The ESC/BEL bytes are removed (neutralizing the escape); the now-inert
        // "[31m" text is harmless. Newlines survive when keep_breaks is true.
        assert_eq!(strip_controls("a\x1b[31mb\x07\nc", true), "a[31mb\nc");
        assert_eq!(strip_controls("a\x1bb\nc", false), "abc");
    }
}
