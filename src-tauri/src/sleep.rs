use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

static ACTIVE_SESSIONS: AtomicI32 = AtomicI32::new(0);
static SLEEP_DISABLED: AtomicBool = AtomicBool::new(false);

const SUDOERS_PATH: &str = "/etc/sudoers.d/autodev-sleep";

/// Install a sudoers entry allowing passwordless `pmset disablesleep`.
/// Prompts for admin credentials via macOS dialog (one-time only).
async fn ensure_sudoers() -> bool {
    if std::path::Path::new(SUDOERS_PATH).exists() {
        return true;
    }

    // Write sudoers rules to a temp file, then move into place with admin privileges
    let tmp = "/tmp/autodev-sleep-sudoers";
    let content = "%admin ALL=(root) NOPASSWD: /usr/bin/pmset disablesleep 0\n\
                   %admin ALL=(root) NOPASSWD: /usr/bin/pmset disablesleep 1\n";
    if std::fs::write(tmp, content).is_err() {
        eprintln!("[sleep] Failed to write temp sudoers file");
        return false;
    }

    let script = format!(
        "do shell script \"mv {tmp} {SUDOERS_PATH} && chown root:wheel {SUDOERS_PATH} && chmod 0440 {SUDOERS_PATH}\" with administrator privileges"
    );

    let result = tokio::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .await;

    match result {
        Ok(o) if o.status.success() => {
            eprintln!("[sleep] Installed sudoers entry for passwordless pmset");
            true
        }
        _ => {
            eprintln!("[sleep] User cancelled admin auth or install failed");
            let _ = std::fs::remove_file(tmp);
            false
        }
    }
}

/// Call when a session starts. Enables sleep prevention if this is the first
/// active session and the setting is on. Only prompts for admin auth on the
/// very first invocation (to install the sudoers entry).
pub async fn on_session_start(sleep_prevention_enabled: bool) {
    let prev = ACTIVE_SESSIONS.fetch_add(1, Ordering::SeqCst);
    if prev == 0 && sleep_prevention_enabled {
        if ensure_sudoers().await {
            set_sleep_disabled(true).await;
        }
    }
}

/// Call when a session ends (completed or failed). Re-enables system sleep
/// when the last active session finishes.
pub async fn on_session_end() {
    let prev = ACTIVE_SESSIONS.fetch_sub(1, Ordering::SeqCst);
    if prev == 1 && SLEEP_DISABLED.load(Ordering::SeqCst) {
        set_sleep_disabled(false).await;
    }
}

/// Best-effort disable on app exit. Synchronous, no admin prompt.
/// If sudoers entry exists, this succeeds silently. If not, `pmset disablesleep`
/// resets on reboot anyway.
pub fn force_disable() {
    if SLEEP_DISABLED.swap(false, Ordering::SeqCst) {
        ACTIVE_SESSIONS.store(0, Ordering::SeqCst);
        let _ = std::process::Command::new("sudo")
            .args(["pmset", "disablesleep", "0"])
            .output();
    }
}

async fn set_sleep_disabled(disabled: bool) {
    let value = if disabled { "1" } else { "0" };
    let result = tokio::process::Command::new("sudo")
        .args(["pmset", "disablesleep", value])
        .output()
        .await;

    match result {
        Ok(o) if o.status.success() => {
            SLEEP_DISABLED.store(disabled, Ordering::SeqCst);
            eprintln!(
                "[sleep] pmset disablesleep {value} — sleep prevention {}",
                if disabled { "enabled" } else { "disabled" }
            );
        }
        Ok(o) => {
            eprintln!(
                "[sleep] pmset disablesleep {value} failed: {}",
                String::from_utf8_lossy(&o.stderr)
            );
        }
        Err(e) => {
            eprintln!("[sleep] Failed to run sudo pmset: {e}");
        }
    }
}
