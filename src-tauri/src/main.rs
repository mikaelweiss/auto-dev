// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // macOS GUI apps launched from Dock/Finder only get a minimal PATH
    // (/usr/bin:/bin:/usr/sbin:/sbin). This reads the user's shell profile
    // and applies the full PATH so spawned subprocesses (claude, gh, git, etc.)
    // can find all CLI tools.
    let _ = fix_path_env::fix();
    auto_dev::run()
}
