use std::io;
use std::path::{Path, PathBuf};

const BEGIN: &str = "# >>> meditate integration >>>";
const END: &str = "# <<< meditate integration <<<";

pub struct Target {
    pub path: PathBuf,
    pub snippet: String,
    pub label: &'static str,
}

/// Integration points keyed to files in the user's home. Snippets embed the
/// shell-escaped binary path; the install only touches files that exist so it
/// never creates a shell config the user doesn't use.
pub fn targets(home: &Path, binary: &str) -> Vec<Target> {
    let bin = shell_escape(binary);
    vec![
        Target {
            path: home.join(".zshrc"),
            snippet: zsh_snippet(&bin),
            label: "zsh",
        },
        Target {
            path: home.join(".bashrc"),
            snippet: bash_snippet(&bin),
            label: "bash",
        },
        Target {
            path: home.join(".tmux.conf"),
            snippet: tmux_snippet(&bin),
            label: "tmux",
        },
    ]
}

pub fn install(home: &Path, binary: &str) -> io::Result<Vec<PathBuf>> {
    let mut changed = Vec::new();
    for target in targets(home, binary) {
        if target.path.exists() {
            apply_install(&target.path, &target.snippet)?;
            changed.push(target.path);
        }
    }
    Ok(changed)
}

pub fn uninstall(home: &Path, binary: &str) -> io::Result<Vec<PathBuf>> {
    let mut changed = Vec::new();
    for target in targets(home, binary) {
        if target.path.exists() {
            apply_uninstall(&target.path)?;
            changed.push(target.path);
        }
    }
    Ok(changed)
}

pub fn apply_install(path: &Path, snippet: &str) -> io::Result<()> {
    let existing = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };
    write_atomic(path, &format!("{}\n", with_block(&existing, snippet)))
}

pub fn apply_uninstall(path: &Path) -> io::Result<()> {
    let existing = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    let cleaned = without_block(&existing);
    let contents = if cleaned.is_empty() {
        String::new()
    } else {
        format!("{cleaned}\n")
    };
    write_atomic(path, &contents)
}

/// Write via a sibling temp file + atomic rename, so a crash mid-write can never
/// truncate the user's shell config.
fn write_atomic(path: &Path, contents: &str) -> io::Result<()> {
    let mut temp = path.as_os_str().to_owned();
    temp.push(".meditate-tmp");
    let temp = PathBuf::from(temp);
    std::fs::write(&temp, contents)?;
    std::fs::rename(&temp, path)
}

/// Replace (or append) the meditate block in `existing`, leaving everything
/// outside the markers untouched. Re-running is idempotent.
pub fn with_block(existing: &str, snippet: &str) -> String {
    let base = without_block(existing);
    let mut out = base;
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(BEGIN);
    out.push('\n');
    out.push_str(snippet.trim());
    out.push('\n');
    out.push_str(END);
    out
}

/// Remove the marker-delimited meditate block, preserving all other lines. Only
/// a properly paired BEGIN..END is stripped; an unmatched BEGIN (e.g. left by a
/// crash mid-write) leaves the file untouched rather than deleting everything
/// after it.
pub fn without_block(existing: &str) -> String {
    let lines: Vec<&str> = existing.lines().collect();
    let begin = lines.iter().position(|line| line.trim() == BEGIN);
    let end = begin.and_then(|b| {
        lines[b + 1..]
            .iter()
            .position(|line| line.trim() == END)
            .map(|offset| b + 1 + offset)
    });
    match (begin, end) {
        (Some(b), Some(e)) => {
            let mut kept: Vec<&str> = lines[..b].to_vec();
            kept.extend_from_slice(&lines[e + 1..]);
            kept.join("\n").trim_end_matches('\n').to_string()
        }
        _ => existing.trim_end_matches('\n').to_string(),
    }
}

/// Single-quote a string for safe embedding in a shell snippet.
pub fn shell_escape(value: &str) -> String {
    let mut out = String::from("'");
    for ch in value.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

fn zsh_snippet(bin: &str) -> String {
    format!(
        "# Suggest a breath after a long command (zsh).\n\
         __meditate_bin={bin}\n\
         __meditate_threshold=120\n\
         __meditate_cooldown=900\n\
         typeset -g __meditate_start=0 __meditate_last=0\n\
         __meditate_preexec() {{ __meditate_start=$SECONDS }}\n\
         __meditate_precmd() {{\n\
         \x20 local dur=$(( SECONDS - __meditate_start ))\n\
         \x20 [[ -n \"$MEDITATE_NUDGE_OFF\" ]] && return\n\
         \x20 command -v pgrep >/dev/null 2>&1 && pgrep -x meditate >/dev/null 2>&1 && return\n\
         \x20 if (( dur >= __meditate_threshold )) && (( SECONDS - __meditate_last >= __meditate_cooldown )); then\n\
         \x20\x20\x20 __meditate_last=$SECONDS\n\
         \x20\x20\x20 print -P \"\\n  that took ${{dur}}s — %F{{green}}$__meditate_bin%f for a breath?\"\n\
         \x20 fi\n\
         }}\n\
         autoload -Uz add-zsh-hook\n\
         add-zsh-hook preexec __meditate_preexec\n\
         add-zsh-hook precmd __meditate_precmd"
    )
}

fn bash_snippet(bin: &str) -> String {
    format!(
        "# Suggest a breath after a long command (bash).\n\
         __meditate_bin={bin}\n\
         __meditate_threshold=120\n\
         __meditate_cooldown=900\n\
         __meditate_last=0\n\
         __meditate_preexec() {{ [[ -n \"$__meditate_running\" ]] || __meditate_start=$SECONDS; __meditate_running=1; }}\n\
         trap '__meditate_preexec' DEBUG\n\
         __meditate_precmd() {{\n\
         \x20 local dur=$(( SECONDS - ${{__meditate_start:-$SECONDS}} ))\n\
         \x20 __meditate_running=\n\
         \x20 [[ -n \"$MEDITATE_NUDGE_OFF\" ]] && return\n\
         \x20 command -v pgrep >/dev/null 2>&1 && pgrep -x meditate >/dev/null 2>&1 && return\n\
         \x20 if (( dur >= __meditate_threshold )) && (( SECONDS - __meditate_last >= __meditate_cooldown )); then\n\
         \x20\x20\x20 __meditate_last=$SECONDS\n\
         \x20\x20\x20 printf '\\n  that took %ss — %s for a breath?\\n' \"$dur\" \"$__meditate_bin\"\n\
         \x20 fi\n\
         }}\n\
         case \"$PROMPT_COMMAND\" in *__meditate_precmd*) ;; *) PROMPT_COMMAND=\"__meditate_precmd;${{PROMPT_COMMAND}}\";; esac"
    )
}

fn tmux_snippet(bin: &str) -> String {
    format!(
        "# prefix + b opens a breath in a popup.\n\
         bind-key b display-popup -E {bin}"
    )
}
