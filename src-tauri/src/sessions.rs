use std::process::Stdio;

use serde_json::Value;
use tauri::{Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::db;
use crate::types::*;
use crate::worktrees;
use crate::AppState;

/// Find the claude CLI binary.
fn find_claude() -> Result<String, String> {
    for path in &[
        "/usr/local/bin/claude",
        "/opt/homebrew/bin/claude",
    ] {
        if std::path::Path::new(path).exists() {
            return Ok(path.to_string());
        }
    }

    // Fallback: use `which`
    let output = std::process::Command::new("/usr/bin/which")
        .arg("claude")
        .output()
        .map_err(|e| format!("Failed to run which: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err("claude CLI not found. Install it from https://claude.ai/cli".to_string())
    }
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn session_start(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    repo_id: i64,
    issue_number: i64,
) -> Result<Session, String> {
    let claude_path = find_claude()?;

    // Prevent duplicate sessions
    {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        if let Some(active) = db::get_active_session(&db, repo_id, issue_number)? {
            return Err(format!(
                "Issue #{issue_number} already has an active {} session",
                active.stage
            ));
        }
    }

    // Get repo config
    let repo = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_repo_by_id(&db, repo_id)?
            .ok_or_else(|| format!("Repo {repo_id} not found"))?
    };

    // Get the spec prompt
    let spec_prompt = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_prompt(&db, "spec")?
            .map(|p| p.prompt_text)
            .unwrap_or_else(|| "Analyze this issue and write a spec.".to_string())
    };

    // Determine repo local path (parent of worktree_dir, or infer)
    // For worktrees we need the actual repo path on disk.
    // We'll store it as a setting per-repo, but for now use a convention:
    // The user should have the repo cloned somewhere. We'll ask for it via the repo config.
    // For now, we use the worktree_dir relative to a repo_path setting.
    let repo_path = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_setting(&db, &format!("repo_{repo_id}_path"))?
            .ok_or_else(|| {
                "Repository local path not configured. Set it in repo settings.".to_string()
            })?
    };

    // Create worktree
    let worktree_path = worktrees::create_worktree(
        &repo_path,
        issue_number,
        &repo.branch_prefix,
        &repo.worktree_dir,
        &repo.base_branch,
    )
    .await?;

    // Run setup script
    if !repo.setup_script.is_empty() {
        worktrees::run_setup_script(&worktree_path, &repo.setup_script).await?;
    }

    // Create session in DB
    let session = Session {
        id: "0".to_string(), // will be replaced
        repo_id,
        issue_number,
        stage: "spec".to_string(),
        worktree_path: Some(worktree_path.clone()),
        session_id: Some(uuid::Uuid::new_v4().to_string()),
        status: "running".to_string(),
        error_message: None,
        started_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
    };

    let session_db_id = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::insert_session(&db, &session)?
    };

    let session = Session {
        id: session_db_id.to_string(),
        ..session
    };

    // Emit session status
    let _ = app_handle.emit("session-status", &session);

    // Spawn claude in background
    let app = app_handle.clone();
    let wt_path = worktree_path.clone();
    let user_prompt = format!(
        "GitHub Issue #{issue_number}\n\nAnalyze this issue and the codebase, then write a detailed spec comment."
    );

    tokio::spawn(async move {
        let result = run_claude_session(
            &claude_path,
            &wt_path,
            &spec_prompt,
            &user_prompt,
            "plan",
            session_db_id,
            &app,
        )
        .await;

        match result {
            Ok(output) => {
                update_status_via_app(&app, session_db_id, "completed", None);

                let _ = app.emit(
                    "session-log",
                    SessionLogEvent {
                        session_id: session_db_id.to_string(),
                        entry: SessionLogEntry {
                            id: uuid::Uuid::new_v4().to_string(),
                            session_id: session_db_id.to_string(),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            event_type: "status_change".to_string(),
                            content: "Spec stage completed".to_string(),
                        },
                    },
                );

                // Check if output contains questions
                let has_questions = output.to_lowercase().contains("question")
                    || output.to_lowercase().contains("?");

                if has_questions {
                    let _ = app.emit(
                        "session-blocked",
                        serde_json::json!({
                            "session_id": session_db_id.to_string(),
                            "question": output,
                        }),
                    );
                }
            }
            Err(e) => {
                update_status_via_app(&app, session_db_id, "failed", Some(&e));

                let _ = app.emit(
                    "session-error",
                    serde_json::json!({
                        "session_id": session_db_id.to_string(),
                        "error": e,
                    }),
                );
            }
        }
    });

    Ok(session)
}

#[tauri::command]
pub async fn session_start_implement(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    repo_id: i64,
    issue_number: i64,
) -> Result<Session, String> {
    let claude_path = find_claude()?;

    // Prevent duplicate sessions
    {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        if let Some(active) = db::get_active_session(&db, repo_id, issue_number)? {
            return Err(format!(
                "Issue #{issue_number} already has an active {} session",
                active.stage
            ));
        }
    }

    let repo = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_repo_by_id(&db, repo_id)?
            .ok_or_else(|| format!("Repo {repo_id} not found"))?
    };

    let implement_prompt = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_prompt(&db, "implement")?
            .map(|p| p.prompt_text)
            .unwrap_or_else(|| "Implement the feature as specified.".to_string())
    };

    // Find existing worktree path from previous session
    let worktree_path = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_latest_session(&db, repo_id, issue_number)?
            .and_then(|s| s.worktree_path)
            .ok_or_else(|| "No worktree found from spec stage".to_string())?
    };

    let session = Session {
        id: "0".to_string(),
        repo_id,
        issue_number,
        stage: "implement".to_string(),
        worktree_path: Some(worktree_path.clone()),
        session_id: Some(uuid::Uuid::new_v4().to_string()),
        status: "running".to_string(),
        error_message: None,
        started_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
    };

    let session_db_id = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::insert_session(&db, &session)?
    };

    let session = Session {
        id: session_db_id.to_string(),
        ..session
    };

    let _ = app_handle.emit("session-status", &session);

    let app = app_handle.clone();
    let wt_path = worktree_path.clone();
    let user_prompt = format!(
        "GitHub Issue #{issue_number}\n\nImplement the feature described in the issue and spec. Write clean, well-tested code."
    );
    let _base_branch = repo.base_branch.clone();

    tokio::spawn(async move {
        let result = run_claude_session(
            &claude_path,
            &wt_path,
            &implement_prompt,
            &user_prompt,
            "bypassPermissions",
            session_db_id,
            &app,
        )
        .await;

        match result {
            Ok(_output) => {
                update_status_via_app(&app, session_db_id, "completed", None);

                let _ = app.emit(
                    "session-log",
                    SessionLogEvent {
                        session_id: session_db_id.to_string(),
                        entry: SessionLogEntry {
                            id: uuid::Uuid::new_v4().to_string(),
                            session_id: session_db_id.to_string(),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            event_type: "status_change".to_string(),
                            content: "Implementation completed, advancing to review".to_string(),
                        },
                    },
                );

                // Auto-advance to review
                let _ = app.emit(
                    "session-advance",
                    serde_json::json!({
                        "session_id": session_db_id.to_string(),
                        "next_stage": "review",
                    }),
                );
            }
            Err(e) => {
                update_status_via_app(&app, session_db_id, "failed", Some(&e));

                let _ = app.emit(
                    "session-error",
                    serde_json::json!({
                        "session_id": session_db_id.to_string(),
                        "error": e,
                    }),
                );
            }
        }
    });

    Ok(session)
}

#[tauri::command]
pub async fn session_start_review(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    repo_id: i64,
    issue_number: i64,
) -> Result<Session, String> {
    let claude_path = find_claude()?;

    // Prevent duplicate sessions
    {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        if let Some(active) = db::get_active_session(&db, repo_id, issue_number)? {
            return Err(format!(
                "Issue #{issue_number} already has an active {} session",
                active.stage
            ));
        }
    }

    let repo = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_repo_by_id(&db, repo_id)?
            .ok_or_else(|| format!("Repo {repo_id} not found"))?
    };

    let review_prompt = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_prompt(&db, "review")?
            .map(|p| p.prompt_text)
            .unwrap_or_else(|| "Review the diff and fix any issues.".to_string())
    };

    let worktree_path = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_latest_session(&db, repo_id, issue_number)?
            .and_then(|s| s.worktree_path)
            .ok_or_else(|| "No worktree found from implement stage".to_string())?
    };

    // Get diff for review
    let diff = worktrees::get_worktree_diff(&worktree_path, &repo.base_branch).await?;

    let session = Session {
        id: "0".to_string(),
        repo_id,
        issue_number,
        stage: "review".to_string(),
        worktree_path: Some(worktree_path.clone()),
        session_id: Some(uuid::Uuid::new_v4().to_string()),
        status: "running".to_string(),
        error_message: None,
        started_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
    };

    let session_db_id = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::insert_session(&db, &session)?
    };

    let session = Session {
        id: session_db_id.to_string(),
        ..session
    };

    let _ = app_handle.emit("session-status", &session);

    let app = app_handle.clone();
    let wt_path = worktree_path.clone();
    let owner = repo.owner.clone();
    let name = repo.name.clone();
    let _base_branch = repo.base_branch.clone();
    let branch_prefix = repo.branch_prefix.clone();
    let user_prompt = format!(
        "Review this diff and fix any issues:\n\n```diff\n{diff}\n```"
    );

    tokio::spawn(async move {
        let result = run_claude_session(
            &claude_path,
            &wt_path,
            &review_prompt,
            &user_prompt,
            "bypassPermissions",
            session_db_id,
            &app,
        )
        .await;

        match result {
            Ok(_output) => {
                // Push and create PR
                let branch_name = format!("{branch_prefix}issue-{issue_number}");

                if let Err(e) = worktrees::push_worktree(&wt_path, &branch_name).await {
                    let msg = format!("Failed to push: {e}");
                    update_status_via_app(&app, session_db_id, "failed", Some(&msg));
                    let _ = app.emit(
                        "session-error",
                        serde_json::json!({
                            "session_id": session_db_id.to_string(),
                            "error": msg,
                        }),
                    );
                    return;
                }

                update_status_via_app(&app, session_db_id, "completed", None);

                let _ = app.emit(
                    "session-log",
                    SessionLogEvent {
                        session_id: session_db_id.to_string(),
                        entry: SessionLogEntry {
                            id: uuid::Uuid::new_v4().to_string(),
                            session_id: session_db_id.to_string(),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            event_type: "status_change".to_string(),
                            content: format!(
                                "Review completed. Pushed to {branch_name}. PR ready for {owner}/{name}."
                            ),
                        },
                    },
                );
            }
            Err(e) => {
                update_status_via_app(&app, session_db_id, "failed", Some(&e));

                let _ = app.emit(
                    "session-error",
                    serde_json::json!({
                        "session_id": session_db_id.to_string(),
                        "error": e,
                    }),
                );
            }
        }
    });

    Ok(session)
}

#[tauri::command]
pub async fn session_respond(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    session_id: String,
    message: String,
) -> Result<(), String> {
    let claude_path = find_claude()?;

    // Parse session DB id
    let session_db_id: i64 = session_id
        .parse()
        .map_err(|_| "Invalid session ID".to_string())?;

    let (repo_id, issue_number, worktree_path, stage) = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        let mut stmt = db
            .prepare(
                "SELECT repo_id, issue_number, worktree_path, stage FROM sessions WHERE id = ?1",
            )
            .map_err(|e| format!("Query error: {e}"))?;

        stmt.query_row(rusqlite::params![session_db_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| format!("Session not found: {e}"))?
    };

    let worktree_path = worktree_path.ok_or("No worktree for this session")?;

    let prompt = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_prompt(&db, &stage)?
            .map(|p| p.prompt_text)
            .unwrap_or_default()
    };

    let permission_mode = if stage == "spec" {
        "plan"
    } else {
        "bypassPermissions"
    };

    // Create new session entry for the resumed work
    let new_session = Session {
        id: "0".to_string(),
        repo_id,
        issue_number,
        stage: stage.clone(),
        worktree_path: Some(worktree_path.clone()),
        session_id: Some(uuid::Uuid::new_v4().to_string()),
        status: "running".to_string(),
        error_message: None,
        started_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
    };

    let new_db_id = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::insert_session(&db, &new_session)?
    };

    let app = app_handle.clone();
    let wt_path = worktree_path;

    tokio::spawn(async move {
        let result = run_claude_session(
            &claude_path,
            &wt_path,
            &prompt,
            &message,
            permission_mode,
            new_db_id,
            &app,
        )
        .await;

        match result {
            Ok(_) => {
                update_status_via_app(&app, new_db_id, "completed", None);

                let _ = app.emit(
                    "session-log",
                    SessionLogEvent {
                        session_id: new_db_id.to_string(),
                        entry: SessionLogEntry {
                            id: uuid::Uuid::new_v4().to_string(),
                            session_id: new_db_id.to_string(),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            event_type: "status_change".to_string(),
                            content: "Session resumed and completed".to_string(),
                        },
                    },
                );
            }
            Err(e) => {
                update_status_via_app(&app, new_db_id, "failed", Some(&e));

                let _ = app.emit(
                    "session-error",
                    serde_json::json!({
                        "session_id": new_db_id.to_string(),
                        "error": e,
                    }),
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn session_retry(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    session_id: String,
) -> Result<Session, String> {
    let session_db_id: i64 = session_id
        .parse()
        .map_err(|_| "Invalid session ID".to_string())?;

    let (repo_id, issue_number, stage) = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        let mut stmt = db
            .prepare("SELECT repo_id, issue_number, stage FROM sessions WHERE id = ?1")
            .map_err(|e| format!("Query error: {e}"))?;

        stmt.query_row(rusqlite::params![session_db_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("Session not found: {e}"))?
    };

    // Restart the appropriate stage
    match stage.as_str() {
        "spec" => session_start(state, app_handle, repo_id, issue_number).await,
        "implement" => session_start_implement(state, app_handle, repo_id, issue_number).await,
        "review" => session_start_review(state, app_handle, repo_id, issue_number).await,
        _ => Err(format!("Unknown stage: {stage}")),
    }
}

#[tauri::command]
pub async fn session_stop(
    _state: tauri::State<'_, AppState>,
    _session_id: String,
) -> Result<(), String> {
    // In a full implementation, we'd track child PIDs and kill them.
    // For now, this is a placeholder that the frontend can call.
    // The spawned tokio tasks will complete on their own.
    // TODO: Track child process handles in AppState for proper cancellation
    Ok(())
}

#[tauri::command]
pub async fn session_run_test(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    session_id: String,
) -> Result<String, String> {
    let session_db_id: i64 = session_id
        .parse()
        .map_err(|_| "Invalid session ID".to_string())?;

    let (repo_id, worktree_path) = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        let mut stmt = db
            .prepare("SELECT repo_id, worktree_path FROM sessions WHERE id = ?1")
            .map_err(|e| format!("Query error: {e}"))?;

        stmt.query_row(rusqlite::params![session_db_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
            ))
        })
        .map_err(|e| format!("Session not found: {e}"))?
    };

    let worktree_path = worktree_path.ok_or("No worktree for this session")?;

    let repo = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_repo_by_id(&db, repo_id)?
            .ok_or_else(|| format!("Repo {repo_id} not found"))?
    };

    worktrees::run_test_script(&worktree_path, &repo.run_script, &app_handle, &session_id).await
}

// ── Settings Commands ───────────────────────────────────────────────────

#[tauri::command]
pub async fn settings_get(state: tauri::State<'_, AppState>) -> Result<AppSettings, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    db::get_app_settings(&db)
}

#[tauri::command]
pub async fn settings_set(
    state: tauri::State<'_, AppState>,
    settings: AppSettings,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    db::save_app_settings(&db, &settings)
}

#[tauri::command]
pub async fn prompts_get(state: tauri::State<'_, AppState>) -> Result<Vec<AgentPrompt>, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    db::get_all_prompts(&db)
}

#[tauri::command]
pub async fn prompts_set(
    state: tauri::State<'_, AppState>,
    stage: String,
    prompt_text: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    db::update_prompt(&db, &stage, &prompt_text)
}

#[tauri::command]
pub async fn set_repo_path(
    state: tauri::State<'_, AppState>,
    repo_id: i64,
    path: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    db::set_setting(&db, &format!("repo_{repo_id}_path"), &path)
}

#[tauri::command]
pub async fn get_repo_path(
    state: tauri::State<'_, AppState>,
    repo_id: i64,
) -> Result<Option<String>, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    db::get_setting(&db, &format!("repo_{repo_id}_path"))
}

// ── Caffeinate ──────────────────────────────────────────────────────────

#[tauri::command]
pub async fn start_caffeinate(pid: u32) -> Result<u32, String> {
    let child = std::process::Command::new("caffeinate")
        .args(["-i", "-w", &pid.to_string()])
        .spawn()
        .map_err(|e| format!("Failed to start caffeinate: {e}"))?;

    Ok(child.id())
}

#[tauri::command]
pub async fn stop_caffeinate(pid: u32) -> Result<(), String> {
    std::process::Command::new("kill")
        .arg(pid.to_string())
        .output()
        .map_err(|e| format!("Failed to stop caffeinate: {e}"))?;
    Ok(())
}

// ── Session Logs & Cleanup ──────────────────────────────────────────────

#[tauri::command]
pub async fn session_get_logs(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<Vec<SessionLogEntry>, String> {
    let session_db_id: i64 = session_id
        .parse()
        .map_err(|_| "Invalid session ID".to_string())?;
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    db::get_session_logs(&db, session_db_id)
}

#[tauri::command]
pub async fn session_cleanup(
    state: tauri::State<'_, AppState>,
    repo_id: i64,
    issue_number: i64,
) -> Result<(), String> {
    let repo = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_repo_by_id(&db, repo_id)?
            .ok_or_else(|| format!("Repo {repo_id} not found"))?
    };

    let repo_path = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_setting(&db, &format!("repo_{repo_id}_path"))?
            .ok_or_else(|| "Repository local path not configured".to_string())?
    };

    let slug = format!("issue-{issue_number}");
    let branch_name = format!("{}issue-{issue_number}", repo.branch_prefix);
    let worktree_path = std::path::Path::new(&repo_path)
        .join(&repo.worktree_dir)
        .join(&slug)
        .to_string_lossy()
        .to_string();

    worktrees::remove_worktree(&repo_path, &worktree_path, &branch_name).await
}

// ── Internal Helpers ────────────────────────────────────────────────────

fn update_status_via_app(
    app: &tauri::AppHandle,
    session_db_id: i64,
    status: &str,
    error: Option<&str>,
) {
    let state = app.state::<AppState>();
    let Ok(db) = state.db.lock() else { return };
    let _ = db::update_session_status(&db, session_db_id, status, error);
}

fn insert_log_via_app(
    app: &tauri::AppHandle,
    session_db_id: i64,
    event_type: &str,
    content: &str,
) {
    let state = app.state::<AppState>();
    let Ok(db) = state.db.lock() else { return };
    let _ = db::insert_session_log(&db, session_db_id, event_type, content);
}

// ── Internal: Run Claude Session ────────────────────────────────────────

async fn run_claude_session(
    claude_path: &str,
    worktree_path: &str,
    system_prompt: &str,
    user_prompt: &str,
    permission_mode: &str,
    session_db_id: i64,
    app_handle: &tauri::AppHandle,
) -> Result<String, String> {
    let mut child = tokio::process::Command::new(claude_path)
        .args([
            "-p",
            "--output-format",
            "stream-json",
            "--permission-mode",
            permission_mode,
        ])
        .arg("--system-prompt")
        .arg(system_prompt)
        .arg(user_prompt)
        .current_dir(worktree_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn claude: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or("Failed to capture claude stdout")?;

    let mut reader = BufReader::new(stdout).lines();
    let mut full_output = String::new();

    while let Some(line) = reader
        .next_line()
        .await
        .map_err(|e| format!("Failed to read claude output: {e}"))?
    {
        // Try to parse as JSON for structured events
        let event_type;
        let content;

        if let Ok(json) = serde_json::from_str::<Value>(&line) {
            event_type = json["type"]
                .as_str()
                .unwrap_or("message")
                .to_string();

            // Extract the text content from various event types
            content = if let Some(text) = json["content"].as_str() {
                text.to_string()
            } else if let Some(result) = json["result"].as_str() {
                result.to_string()
            } else {
                line.clone()
            };
        } else {
            event_type = "message".to_string();
            content = line.clone();
        }

        full_output.push_str(&content);
        full_output.push('\n');

        // Persist log to DB
        insert_log_via_app(app_handle, session_db_id, &event_type, &content);

        // Emit log event to frontend
        let _ = app_handle.emit(
            "session-log",
            SessionLogEvent {
                session_id: session_db_id.to_string(),
                entry: SessionLogEntry {
                    id: uuid::Uuid::new_v4().to_string(),
                    session_id: session_db_id.to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    event_type,
                    content,
                },
            },
        );
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for claude: {e}"))?;

    if !status.success() {
        // Read stderr for error details
        return Err(format!("Claude exited with status {status}"));
    }

    Ok(full_output)
}
