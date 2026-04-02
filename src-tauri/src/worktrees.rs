use std::path::Path;
use tokio::process::Command;

/// Create a git worktree for an issue.
/// Returns the path to the new worktree directory.
/// Worktrees are stored in `~/.autodev/{repo_name}/issue-{number}/`.
pub async fn create_worktree(
    repo_path: &str,
    issue_number: i64,
    branch_prefix: &str,
    repo_name: &str,
    base_branch: &str,
) -> Result<String, String> {
    let slug = format!("issue-{issue_number}");
    let branch_name = format!("{branch_prefix}{slug}");
    let home = std::env::var("HOME")
        .map_err(|_| "HOME environment variable not set".to_string())?;
    let wt_path = Path::new(&home)
        .join(".autodev")
        .join(repo_name)
        .join(&slug);

    // Create the worktree directory parent if needed
    if let Some(parent) = wt_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create worktree directory: {e}"))?;
    }

    let wt_path_str = wt_path
        .to_str()
        .ok_or_else(|| "Invalid worktree path".to_string())?
        .to_string();

    // If the worktree already exists, reuse it — a previous session already set it up
    if wt_path.exists() {
        return Ok(wt_path_str);
    }

    // Fetch latest from remote
    let fetch_output = Command::new("git")
        .args(["fetch", "origin", base_branch])
        .current_dir(repo_path)
        .output()
        .await
        .map_err(|e| format!("Failed to git fetch: {e}"))?;

    if !fetch_output.status.success() {
        let stderr = String::from_utf8_lossy(&fetch_output.stderr);
        eprintln!("git fetch warning: {stderr}");
    }

    // Create worktree with new branch
    let output = Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            &branch_name,
            &wt_path_str,
            &format!("origin/{base_branch}"),
        ])
        .current_dir(repo_path)
        .output()
        .await
        .map_err(|e| format!("Failed to create worktree: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // If branch already exists, try without -b
        if stderr.contains("already exists") {
            let output2 = Command::new("git")
                .args(["worktree", "add", &wt_path_str, &branch_name])
                .current_dir(repo_path)
                .output()
                .await
                .map_err(|e| format!("Failed to create worktree (retry): {e}"))?;

            if !output2.status.success() {
                let stderr2 = String::from_utf8_lossy(&output2.stderr);
                return Err(format!("Failed to create worktree: {stderr2}"));
            }
        } else {
            return Err(format!("Failed to create worktree: {stderr}"));
        }
    }

    Ok(wt_path_str)
}

/// Remove a git worktree and its branch.
pub async fn remove_worktree(
    repo_path: &str,
    worktree_path: &str,
    branch_name: &str,
) -> Result<(), String> {
    // Remove the worktree
    let output = Command::new("git")
        .args(["worktree", "remove", worktree_path, "--force"])
        .current_dir(repo_path)
        .output()
        .await
        .map_err(|e| format!("Failed to remove worktree: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Warning: Failed to remove worktree: {stderr}");
    }

    // Delete the branch
    let output = Command::new("git")
        .args(["branch", "-D", branch_name])
        .current_dir(repo_path)
        .output()
        .await
        .map_err(|e| format!("Failed to delete branch: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Warning: Failed to delete branch {branch_name}: {stderr}");
    }

    Ok(())
}

/// Run a setup script in the worktree directory.
pub async fn run_setup_script(
    worktree_path: &str,
    script: &str,
) -> Result<String, String> {
    if script.trim().is_empty() {
        return Ok("No setup script configured.".to_string());
    }

    let output = Command::new("bash")
        .args(["-c", script])
        .current_dir(worktree_path)
        .output()
        .await
        .map_err(|e| format!("Failed to run setup script: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!("Setup script failed:\nstdout: {stdout}\nstderr: {stderr}"));
    }

    Ok(format!("{stdout}\n{stderr}"))
}

/// Run a test/run script in the worktree directory, streaming output via Tauri events.
pub async fn run_test_script(
    worktree_path: &str,
    script: &str,
    app_handle: &tauri::AppHandle,
    session_id: &str,
) -> Result<String, String> {
    use tauri::Emitter;
    use tokio::io::{AsyncBufReadExt, BufReader};

    if script.trim().is_empty() {
        return Err("No run script configured for this repo.".to_string());
    }

    let mut child = Command::new("bash")
        .args(["-c", script])
        .current_dir(worktree_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run test script: {e}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let app = app_handle.clone();
    let sid = session_id.to_string();

    let mut all_output = String::new();

    if let Some(stdout) = stdout {
        let mut reader = BufReader::new(stdout).lines();
        while let Some(line) = reader
            .next_line()
            .await
            .map_err(|e| format!("Failed to read stdout: {e}"))?
        {
            let _ = app.emit(
                "test-output",
                serde_json::json!({ "session_id": sid, "line": line }),
            );
            all_output.push_str(&line);
            all_output.push('\n');
        }
    }

    if let Some(stderr) = stderr {
        let mut reader = BufReader::new(stderr).lines();
        while let Some(line) = reader
            .next_line()
            .await
            .map_err(|e| format!("Failed to read stderr: {e}"))?
        {
            let _ = app.emit(
                "test-output",
                serde_json::json!({ "session_id": sid, "line": line }),
            );
            all_output.push_str(&line);
            all_output.push('\n');
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for test script: {e}"))?;

    if !status.success() {
        return Err(format!("Test script exited with {status}:\n{all_output}"));
    }

    Ok(all_output)
}

/// Get the diff between the worktree and the base branch.
pub async fn get_worktree_diff(
    worktree_path: &str,
    base_branch: &str,
) -> Result<String, String> {
    let output = Command::new("git")
        .args(["diff", &format!("origin/{base_branch}"), "--", "."])
        .current_dir(worktree_path)
        .output()
        .await
        .map_err(|e| format!("Failed to get diff: {e}"))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Push the worktree's branch to remote.
pub async fn push_worktree(
    worktree_path: &str,
    branch_name: &str,
) -> Result<(), String> {
    let output = Command::new("git")
        .args(["push", "-u", "origin", branch_name])
        .current_dir(worktree_path)
        .output()
        .await
        .map_err(|e| format!("Failed to push: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Try force push if normal push fails (rebased branch)
        let output2 = Command::new("git")
            .args(["push", "-u", "origin", branch_name, "--force-with-lease"])
            .current_dir(worktree_path)
            .output()
            .await
            .map_err(|e| format!("Failed to force push: {e}"))?;

        if !output2.status.success() {
            let stderr2 = String::from_utf8_lossy(&output2.stderr);
            return Err(format!("Failed to push: {stderr}\nForce push also failed: {stderr2}"));
        }
    }

    Ok(())
}
