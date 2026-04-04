use std::process::Stdio;

use tauri::{Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::db;
use crate::sdk_types::CliMessage;
use crate::types::*;
use crate::worktrees;
use crate::AppState;

/// Find the claude CLI binary.
fn find_claude() -> Result<String, String> {
    // Check ~/.local/bin first (common install location)
    if let Ok(home) = std::env::var("HOME") {
        let local_path = format!("{home}/.local/bin/claude");
        if std::path::Path::new(&local_path).exists() {
            return Ok(local_path);
        }
    }

    for path in &["/usr/local/bin/claude", "/opt/homebrew/bin/claude"] {
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
pub async fn session_list(state: tauri::State<'_, AppState>) -> Result<Vec<Session>, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    let sessions = db::get_all_sessions(&db)?;
    Ok(sessions)
}

#[tauri::command]
pub async fn session_hide(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let session_db_id: i64 = session_id
        .parse()
        .map_err(|_| "Invalid session ID".to_string())?;
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    db::hide_session(&db, session_db_id)
}

#[tauri::command]
pub async fn session_unhide(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let session_db_id: i64 = session_id
        .parse()
        .map_err(|_| "Invalid session ID".to_string())?;
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    db::unhide_session(&db, session_db_id)
}

#[tauri::command]
pub async fn session_list_hidden(
    state: tauri::State<'_, AppState>,
    repo_id: i64,
    issue_number: i64,
) -> Result<Vec<Session>, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    db::get_hidden_sessions(&db, repo_id, issue_number)
}

/// Helper to emit a session-status event to the frontend.
fn emit_session_status(app_handle: &tauri::AppHandle, session: &Session) {
    let _ = app_handle.emit("session-status", session);
}

#[tauri::command]
pub async fn session_start(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    repo_id: i64,
    issue_number: i64,
) -> Result<Session, String> {
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

    // ── Create session IMMEDIATELY so the card is pinned ──
    let session = Session {
        id: "0".to_string(),
        repo_id,
        issue_number,
        stage: "spec".to_string(),
        worktree_path: None,
        session_id: None,
        status: "initializing".to_string(),
        error_message: None,
        started_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
        hidden: false,
        cost_usd: None,
    };

    let session_db_id = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::insert_session(&db, &session)?
    };

    let mut session = Session {
        id: session_db_id.to_string(),
        ..session
    };

    // Emit immediately — card is now pinned to "claimed"
    emit_session_status(&app_handle, &session);

    // Helper macro: on failure, mark session as failed and return
    macro_rules! fail_session {
        ($msg:expr) => {{
            session.status = "failed".to_string();
            session.error_message = Some($msg.clone());
            update_status_via_app(&app_handle, session_db_id, "failed", Some(&$msg));
            emit_session_status(&app_handle, &session);
            return Ok(session);
        }};
    }

    // ── Validate preconditions ──
    let claude_path = match find_claude() {
        Ok(p) => p,
        Err(e) => fail_session!(e),
    };

    let repo = match {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_repo_by_id(&db, repo_id)?
    } {
        Some(r) => r,
        None => fail_session!(format!("Repo {repo_id} not found")),
    };

    let (spec_prompt, stage_model, stage_effort) = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        match db::get_prompt(&db, "spec")? {
            Some(p) => (p.prompt_text, p.model, p.effort),
            None => (
                "Analyze this issue and write a spec.".to_string(),
                "haiku".to_string(),
                "high".to_string(),
            ),
        }
    };

    let repo_path = match {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_setting(&db, &format!("repo_{repo_id}_path"))?
    } {
        Some(p) => p,
        None => fail_session!(
            "Repository local path not configured. Set it in Settings > Repository.".to_string()
        ),
    };

    // ── Create worktree ──
    let worktree_path = match worktrees::create_worktree(
        &repo_path,
        issue_number,
        &repo.branch_prefix,
        &repo.name,
        &repo.base_branch,
    )
    .await
    {
        Ok(path) => path,
        Err(e) => fail_session!(format!("Worktree creation failed: {e}")),
    };

    session.worktree_path = Some(worktree_path.clone());

    // ── Run setup script ──
    if !repo.setup_script.is_empty() {
        session.status = "setup".to_string();
        emit_session_status(&app_handle, &session);

        if let Err(e) = worktrees::run_setup_script(&worktree_path, &repo.setup_script).await {
            fail_session!(format!("Setup script failed: {e}"));
        }
    }

    // ── Update to running ──
    session.status = "running".to_string();
    {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::update_session_status(&db, session_db_id, "running", None)?;
        db.execute(
            "UPDATE sessions SET worktree_path = ?1 WHERE id = ?2",
            rusqlite::params![worktree_path, session_db_id],
        )
        .map_err(|e| format!("Failed to update worktree path: {e}"))?;
    }
    emit_session_status(&app_handle, &session);

    // Read settings for sleep prevention and permissions
    let (sleep_enabled, permission_mode) = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        let settings = db::get_app_settings(&db)?;
        let mode = if settings.bypass_permissions {
            "bypassPermissions".to_string()
        } else {
            "auto".to_string()
        };
        (settings.sleep_prevention, mode)
    };
    crate::sleep::on_session_start(sleep_enabled).await;

    // Spawn claude in background
    let app = app_handle.clone();
    let wt_path = worktree_path.clone();
    let owner = repo.owner.clone();
    let name = repo.name.clone();
    let user_prompt = format!(
        "GitHub Issue #{issue_number} in {owner}/{name}\n\n\
         Follow the spec process: check for an existing spec in the issue comments, \
         explore the codebase, and produce or update the spec. \
         Use `gh` CLI for all GitHub interactions (comments, labels). \
         The repo is {owner}/{name} and the issue number is {issue_number}."
    );

    tokio::spawn(async move {
        let result = run_claude_session(
            &claude_path,
            &wt_path,
            &spec_prompt,
            &user_prompt,
            &permission_mode,
            &stage_model,
            &stage_effort,
            None,
            session_db_id,
            &app,
        )
        .await;

        match result {
            Ok(res) => {
                if let Some(ref cli_id) = res.cli_session_id {
                    save_cli_session_id(&app, session_db_id, cli_id);
                }
                if let Some(cost) = res.cost_usd {
                    save_session_cost(&app, session_db_id, cost);
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
                            content: "Spec stage completed".to_string(),
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

        crate::sleep::on_session_end().await;
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

    let (implement_prompt, stage_model, stage_effort) = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        match db::get_prompt(&db, "implement")? {
            Some(p) => (p.prompt_text, p.model, p.effort),
            None => (
                "Implement the feature as specified.".to_string(),
                "haiku".to_string(),
                "high".to_string(),
            ),
        }
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
        session_id: None,
        status: "running".to_string(),
        error_message: None,
        started_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
        hidden: false,
        cost_usd: None,
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

    // Read settings for sleep prevention and permissions
    let (sleep_enabled, permission_mode) = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        let settings = db::get_app_settings(&db)?;
        let mode = if settings.bypass_permissions {
            "bypassPermissions".to_string()
        } else {
            "auto".to_string()
        };
        (settings.sleep_prevention, mode)
    };
    crate::sleep::on_session_start(sleep_enabled).await;

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
            &permission_mode,
            &stage_model,
            &stage_effort,
            None,
            session_db_id,
            &app,
        )
        .await;

        match result {
            Ok(res) => {
                if let Some(ref cli_id) = res.cli_session_id {
                    save_cli_session_id(&app, session_db_id, cli_id);
                }
                if let Some(cost) = res.cost_usd {
                    save_session_cost(&app, session_db_id, cost);
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

        crate::sleep::on_session_end().await;
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

    let (review_prompt, stage_model, stage_effort) = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        match db::get_prompt(&db, "review")? {
            Some(p) => (p.prompt_text, p.model, p.effort),
            None => (
                "Review the diff and fix any issues.".to_string(),
                "haiku".to_string(),
                "high".to_string(),
            ),
        }
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
        session_id: None,
        status: "running".to_string(),
        error_message: None,
        started_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
        hidden: false,
        cost_usd: None,
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

    // Read settings for sleep prevention and permissions
    let (sleep_enabled, permission_mode) = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        let settings = db::get_app_settings(&db)?;
        let mode = if settings.bypass_permissions {
            "bypassPermissions".to_string()
        } else {
            "auto".to_string()
        };
        (settings.sleep_prevention, mode)
    };
    crate::sleep::on_session_start(sleep_enabled).await;

    let app = app_handle.clone();
    let wt_path = worktree_path.clone();
    let owner = repo.owner.clone();
    let name = repo.name.clone();
    let _base_branch = repo.base_branch.clone();
    let branch_prefix = repo.branch_prefix.clone();
    let user_prompt = format!("Review this diff and fix any issues:\n\n```diff\n{diff}\n```");

    tokio::spawn(async move {
        let result = run_claude_session(
            &claude_path,
            &wt_path,
            &review_prompt,
            &user_prompt,
            &permission_mode,
            &stage_model,
            &stage_effort,
            None,
            session_db_id,
            &app,
        )
        .await;

        match result {
            Ok(res) => {
                if let Some(ref cli_id) = res.cli_session_id {
                    save_cli_session_id(&app, session_db_id, cli_id);
                }
                if let Some(cost) = res.cost_usd {
                    save_session_cost(&app, session_db_id, cost);
                }
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
                    crate::sleep::on_session_end().await;
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

        crate::sleep::on_session_end().await;
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

    let (worktree_path, stage, cli_session_id) = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        let mut stmt = db
            .prepare("SELECT worktree_path, stage, session_id FROM sessions WHERE id = ?1")
            .map_err(|e| format!("Query error: {e}"))?;

        stmt.query_row(rusqlite::params![session_db_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(|e| format!("Session not found: {e}"))?
    };

    let worktree_path = worktree_path.ok_or("No worktree for this session")?;

    let (prompt, stage_model, stage_effort) = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        match db::get_prompt(&db, &stage)? {
            Some(p) => (p.prompt_text, p.model, p.effort),
            None => (String::new(), "haiku".to_string(), "high".to_string()),
        }
    };

    let permission_mode = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        let settings = db::get_app_settings(&db)?;
        if stage == "spec" {
            "plan".to_string()
        } else if settings.bypass_permissions {
            "bypassPermissions".to_string()
        } else {
            "auto".to_string()
        }
    };

    // Update existing session status back to running
    {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::update_session_status(&db, session_db_id, "running", None)?;
    }

    // Notify the frontend about the resumed session
    update_status_via_app(&app_handle, session_db_id, "running", None);

    // Enable sleep prevention if this is the first active session
    let sleep_enabled = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_app_settings(&db)
            .map(|s| s.sleep_prevention)
            .unwrap_or(true)
    };
    crate::sleep::on_session_start(sleep_enabled).await;

    let app = app_handle.clone();
    let wt_path = worktree_path;

    tokio::spawn(async move {
        let result = run_claude_session(
            &claude_path,
            &wt_path,
            &prompt,
            &message,
            &permission_mode,
            &stage_model,
            &stage_effort,
            cli_session_id.as_deref(),
            session_db_id,
            &app,
        )
        .await;

        match result {
            Ok(res) => {
                if let Some(ref cli_id) = res.cli_session_id {
                    save_cli_session_id(&app, session_db_id, cli_id);
                }
                if let Some(cost) = res.cost_usd {
                    save_session_cost(&app, session_db_id, cost);
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
                            content: "Session resumed and completed".to_string(),
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

        crate::sleep::on_session_end().await;
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
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    session_id: String,
) -> Result<(), String> {
    let session_db_id: i64 = session_id
        .parse()
        .map_err(|_| "Invalid session ID".to_string())?;

    // Look up the PID and kill the process
    let pid = {
        let pids = state
            .active_pids
            .lock()
            .map_err(|e| format!("PID lock: {e}"))?;
        pids.get(&session_db_id).copied()
    };

    if let Some(pid) = pid {
        // Send SIGTERM to the process group so child processes also die
        #[cfg(unix)]
        unsafe {
            libc::kill(-(pid as i32), libc::SIGTERM);
        }

        // Mark as completed — this is a user-initiated stop, not a failure
        update_status_via_app(&app_handle, session_db_id, "completed", None);

        let _ = app_handle.emit(
            "session-log",
            SessionLogEvent {
                session_id: session_db_id.to_string(),
                entry: SessionLogEntry {
                    id: uuid::Uuid::new_v4().to_string(),
                    session_id: session_db_id.to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    event_type: "status_change".to_string(),
                    content: "Session stopped".to_string(),
                },
            },
        );
    }

    Ok(())
}

#[tauri::command]
pub async fn session_list_files(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<Vec<String>, String> {
    let session_db_id: i64 = session_id
        .parse()
        .map_err(|_| "Invalid session ID".to_string())?;

    let worktree_path = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        let mut stmt = db
            .prepare("SELECT worktree_path FROM sessions WHERE id = ?1")
            .map_err(|e| format!("Query error: {e}"))?;
        stmt.query_row(rusqlite::params![session_db_id], |row| {
            row.get::<_, Option<String>>(0)
        })
        .map_err(|e| format!("Session not found: {e}"))?
    };

    let worktree_path = worktree_path.ok_or("No worktree for this session")?;

    let output = tokio::process::Command::new("git")
        .args(["ls-files"])
        .current_dir(&worktree_path)
        .output()
        .await
        .map_err(|e| format!("Failed to list files: {e}"))?;

    if !output.status.success() {
        return Err("Failed to list worktree files".to_string());
    }

    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.to_string())
        .collect();

    Ok(files)
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
    model: String,
    effort: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    db::update_prompt(&db, &stage, &prompt_text, &model, &effort)
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

#[tauri::command]
pub async fn get_selected_repo_id(state: tauri::State<'_, AppState>) -> Result<Option<i64>, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    Ok(db::get_setting(&db, "selected_repo_id")?.and_then(|v| v.parse::<i64>().ok()))
}

#[tauri::command]
pub async fn set_selected_repo_id(
    state: tauri::State<'_, AppState>,
    repo_id: i64,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    db::set_setting(&db, "selected_repo_id", &repo_id.to_string())
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

    let home =
        std::env::var("HOME").map_err(|_| "HOME environment variable not set".to_string())?;
    let slug = format!("issue-{issue_number}");
    let branch_name = format!("{}issue-{issue_number}", repo.branch_prefix);
    let worktree_path = std::path::Path::new(&home)
        .join(".autodev")
        .join(&repo.name)
        .join(&slug)
        .to_string_lossy()
        .to_string();

    worktrees::remove_worktree(&repo_path, &worktree_path, &branch_name).await
}

// ── Internal Helpers ────────────────────────────────────────────────────

fn update_status_via_app(app: &tauri::AppHandle, session_db_id: i64, status: &str, error: Option<&str>) {
    let state = app.state::<AppState>();
    let Ok(db) = state.db.lock() else { return };
    let _ = db::update_session_status(&db, session_db_id, status, error);

    // Re-read the session and emit to frontend so the UI updates
    if let Ok(Some(session)) = db::get_session_by_id(&db, session_db_id) {
        let _ = app.emit("session-status", &session);
    }
}

/// Save the captured Claude CLI session ID to the database.
fn save_cli_session_id(app: &tauri::AppHandle, session_db_id: i64, cli_session_id: &str) {
    let state = app.state::<AppState>();
    let Ok(db) = state.db.lock() else { return };
    let _ = db::update_session_cli_id(&db, session_db_id, cli_session_id);
}

/// Save the session cost to the database.
fn save_session_cost(app: &tauri::AppHandle, session_db_id: i64, cost: f64) {
    let state = app.state::<AppState>();
    let Ok(db) = state.db.lock() else { return };
    let _ = db.execute(
        "UPDATE sessions SET cost_usd = ?1 WHERE id = ?2",
        rusqlite::params![cost, session_db_id],
    );
}

fn insert_log_via_app(app: &tauri::AppHandle, session_db_id: i64, event_type: &str, content: &str) {
    let state = app.state::<AppState>();
    let Ok(db) = state.db.lock() else { return };
    let _ = db::insert_session_log(&db, session_db_id, event_type, content);
}

/// Register an active PID so session_stop can kill it.
fn register_pid(app: &tauri::AppHandle, session_db_id: i64, pid: u32) {
    let state = app.state::<AppState>();
    let Ok(mut pids) = state.active_pids.lock() else {
        return;
    };
    pids.insert(session_db_id, pid);
}

/// Remove a PID when the process exits.
fn unregister_pid(app: &tauri::AppHandle, session_db_id: i64) {
    let state = app.state::<AppState>();
    let Ok(mut pids) = state.active_pids.lock() else {
        return;
    };
    pids.remove(&session_db_id);
}

// ── Internal: Run Claude Session ────────────────────────────────────────

/// Result from running a Claude session.
struct ClaudeSessionResult {
    cli_session_id: Option<String>,
    cost_usd: Option<f64>,
}

async fn run_claude_session(
    claude_path: &str,
    worktree_path: &str,
    system_prompt: &str,
    user_prompt: &str,
    permission_mode: &str,
    model: &str,
    effort: &str,
    resume_session_id: Option<&str>,
    session_db_id: i64,
    app_handle: &tauri::AppHandle,
) -> Result<ClaudeSessionResult, String> {
    let mut cmd = tokio::process::Command::new(claude_path);
    cmd.args([
        "-p",
        "--verbose",
        "--output-format",
        "stream-json",
        "--permission-mode",
        permission_mode,
        "--model",
        model,
        "--effort",
        effort,
    ]);

    // Resume a previous conversation if we have a CLI session ID
    if let Some(resume_id) = resume_session_id {
        cmd.arg("--resume").arg(resume_id);
    } else {
        // Only set system prompt for new conversations; resumed ones already have it
        cmd.arg("--system-prompt").arg(system_prompt);
    }

    cmd.arg(user_prompt)
        .current_dir(worktree_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Create a new process group so we can kill all children together
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            libc::setpgid(0, 0);
            Ok(())
        });
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn claude: {e}"))?;

    // Register the PID for cancellation support
    if let Some(pid) = child.id() {
        register_pid(app_handle, session_db_id, pid);
    }

    let stdout = child
        .stdout
        .take()
        .ok_or("Failed to capture claude stdout")?;
    let stderr = child.stderr.take();

    let mut reader = BufReader::new(stdout).lines();
    let mut cli_session_id: Option<String> = None;
    let mut cost_usd: Option<f64> = None;

    while let Some(line) = reader
        .next_line()
        .await
        .map_err(|e| format!("Failed to read claude output: {e}"))?
    {
        let msg = CliMessage::parse(&line);

        // Capture and persist session ID as soon as we see it
        if cli_session_id.is_none() {
            if let Some(sid) = msg.session_id() {
                cli_session_id = Some(sid.to_string());
                save_cli_session_id(app_handle, session_db_id, sid);
            }
        }

        // Capture cost from result messages
        if let CliMessage::Result(ref result) = msg {
            cost_usd = result.total_cost_usd;
        }

        // Convert to log entries and emit
        let entries = msg.to_log_entries();
        for entry in entries {
            insert_log_via_app(app_handle, session_db_id, &entry.event_type, &entry.content);

            let _ = app_handle.emit(
                "session-log",
                SessionLogEvent {
                    session_id: session_db_id.to_string(),
                    entry: SessionLogEntry {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: session_db_id.to_string(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        event_type: entry.event_type,
                        content: entry.content,
                    },
                },
            );
        }
    }

    // Unregister the PID now that the process has exited
    unregister_pid(app_handle, session_db_id);

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for claude: {e}"))?;

    // Exit code 143 = SIGTERM (128+15), meaning user stopped the session.
    // Don't treat this as a failure.
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if status.signal() == Some(libc::SIGTERM) {
            return Ok(ClaudeSessionResult { cli_session_id, cost_usd });
        }
    }

    if !status.success() {
        let mut stderr_output = String::new();
        if let Some(stderr) = stderr {
            let mut stderr_reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = stderr_reader.next_line().await {
                stderr_output.push_str(&line);
                stderr_output.push('\n');
            }
        }
        let detail = if stderr_output.trim().is_empty() {
            format!("Claude exited with {status}")
        } else {
            format!("Claude exited with {status}: {}", stderr_output.trim())
        };
        return Err(detail);
    }

    Ok(ClaudeSessionResult {
        cli_session_id,
        cost_usd,
    })
}
