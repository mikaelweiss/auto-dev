use tauri::{Emitter, Manager};

use crate::db;
use crate::provider::{self, SessionConfig};
use crate::types::*;
use crate::worktrees;
use crate::AppState;

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
    message: Option<String>,
    model_override: Option<String>,
    effort_override: Option<String>,
) -> Result<Session, String> {
    // Get model config from stage defaults, with optional overrides
    let (spec_prompt, stage_model, stage_effort) = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        match db::get_prompt(&db, "spec")? {
            Some(p) => (p.prompt_text, p.model, p.effort),
            None => (
                "Analyze this issue and write a spec.".to_string(),
                "claude-sonnet-4-6".to_string(),
                "high".to_string(),
            ),
        }
    };

    let effective_model = model_override.unwrap_or(stage_model);
    let effective_effort = effort_override.unwrap_or(stage_effort);

    // Resolve provider from model
    let effective_provider = provider::provider_for_model(&effective_model)
        .unwrap_or(provider::ProviderKind::Claude);
    let the_provider = provider::get_provider_by_kind(effective_provider);

    // ── Atomically check for duplicates + create session so the card is pinned ──
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
        provider: effective_provider.as_str().to_string(),
        model: effective_model.clone(),
    };

    let session_db_id = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::check_and_insert_session(&db, &session)?
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
    if let Err(e) = the_provider.find_binary() {
        fail_session!(e);
    }

    let repo = match {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_repo_by_id(&db, repo_id)?
    } {
        Some(r) => r,
        None => fail_session!(format!("Repo {repo_id} not found")),
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

    // Build user prompt
    let app = app_handle.clone();
    let owner = repo.owner.clone();
    let name = repo.name.clone();
    let user_prompt = if let Some(ref msg) = message {
        msg.clone()
    } else {
        format!(
            "GitHub Issue #{issue_number} in {owner}/{name}\n\n\
             Follow the spec process: check for an existing spec in the issue comments, \
             explore the codebase, and produce or update the spec. \
             Use `gh` CLI for all GitHub interactions (comments). \
             The repo is {owner}/{name} and the issue number is {issue_number}."
        )
    };

    // Emit user message to the log if this is a user-typed message
    if message.is_some() {
        emit_user_message(&app_handle, session_db_id, &user_prompt);
    }

    let wt_path = worktree_path.clone();
    let model_for_spawn = effective_model;
    let effort_for_spawn = effective_effort;
    let provider_kind = effective_provider;
    let spec_repo_id = repo_id;
    let spec_issue_number = issue_number;

    let mcp_binary = provider::find_mcp_binary();

    tokio::spawn(async move {
        let prov = provider::get_provider_by_kind(provider_kind);
        let config = SessionConfig {
            worktree_path: wt_path,
            system_prompt: spec_prompt,
            user_prompt,
            permission_mode,
            model: model_for_spawn,
            effort: effort_for_spawn,
            resume_session_id: None,
            mcp_binary_path: mcp_binary,
        };

        let result = provider::run_provider_session(prov.as_ref(), config, session_db_id, &app).await;

        match result {
            Ok(res) => {
                if let Some(cost) = res.cost_usd {
                    save_session_cost(&app, session_db_id, cost);
                }

                match res.signal {
                    Some(provider::StateSignal::AdvanceToInProgress) => {
                        // Spec complete — auto-start implement
                        update_status_via_app(&app, session_db_id, "completed", None);
                        set_issue_column_via_app(&app, spec_repo_id, spec_issue_number, "in_progress");
                        let _ = app.emit(
                            "session-log",
                            SessionLogEvent {
                                session_id: session_db_id.to_string(),
                                entry: SessionLogEntry {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    session_id: session_db_id.to_string(),
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                    event_type: "status_change".to_string(),
                                    content: "Spec complete — starting implementation".to_string(),
                                },
                            },
                        );
                        auto_start_implement(&app, spec_repo_id, spec_issue_number).await;
                    }
                    Some(provider::StateSignal::AdvanceToBlocked) => {
                        // Spec needs human input
                        update_status_via_app(&app, session_db_id, "completed", None);
                        set_issue_column_via_app(&app, spec_repo_id, spec_issue_number, "blocked");
                    }
                    _ => {
                        // No signal — just mark completed
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
    {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_repo_by_id(&db, repo_id)?
            .ok_or_else(|| format!("Repo {repo_id} not found"))?;
    }

    let (implement_prompt, stage_model, stage_effort) = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        match db::get_prompt(&db, "implement")? {
            Some(p) => (p.prompt_text, p.model, p.effort),
            None => (
                "Implement the feature as specified.".to_string(),
                "claude-sonnet-4-6".to_string(),
                "high".to_string(),
            ),
        }
    };

    let effective_provider = provider::provider_for_model(&stage_model)
        .unwrap_or(provider::ProviderKind::Claude);
    let the_provider = provider::get_provider_by_kind(effective_provider);
    the_provider.find_binary()?;

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
        provider: effective_provider.as_str().to_string(),
        model: stage_model.clone(),
    };

    // Atomically check for duplicates + insert
    let session_db_id = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::check_and_insert_session(&db, &session)?
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
    let provider_kind = effective_provider;
    let model_for_spawn = stage_model;
    let effort_for_spawn = stage_effort;
    let mcp_binary = provider::find_mcp_binary();
    let impl_repo_id = repo_id;
    let impl_issue_number = issue_number;

    tokio::spawn(async move {
        let prov = provider::get_provider_by_kind(provider_kind);
        let config = SessionConfig {
            worktree_path: wt_path,
            system_prompt: implement_prompt,
            user_prompt,
            permission_mode,
            model: model_for_spawn,
            effort: effort_for_spawn,
            resume_session_id: None,
            mcp_binary_path: mcp_binary,
        };

        let result = provider::run_provider_session(prov.as_ref(), config, session_db_id, &app).await;
        handle_implement_result(&app, result, session_db_id, impl_repo_id, impl_issue_number).await;
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
                "claude-sonnet-4-6".to_string(),
                "high".to_string(),
            ),
        }
    };

    let effective_provider = provider::provider_for_model(&stage_model)
        .unwrap_or(provider::ProviderKind::Claude);
    let the_provider = provider::get_provider_by_kind(effective_provider);
    the_provider.find_binary()?;

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
        provider: effective_provider.as_str().to_string(),
        model: stage_model.clone(),
    };

    // Atomically check for duplicates + insert
    let session_db_id = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::check_and_insert_session(&db, &session)?
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
    let branch_prefix = repo.branch_prefix.clone();
    let user_prompt = format!("Review this diff and fix any issues:\n\n```diff\n{diff}\n```");
    let provider_kind = effective_provider;
    let model_for_spawn = stage_model;
    let effort_for_spawn = stage_effort;
    let mcp_binary = provider::find_mcp_binary();

    tokio::spawn(async move {
        let prov = provider::get_provider_by_kind(provider_kind);
        let config = SessionConfig {
            worktree_path: wt_path.clone(),
            system_prompt: review_prompt,
            user_prompt,
            permission_mode,
            model: model_for_spawn,
            effort: effort_for_spawn,
            resume_session_id: None,
            mcp_binary_path: mcp_binary,
        };

        let result = provider::run_provider_session(prov.as_ref(), config, session_db_id, &app).await;

        match result {
            Ok(res) => {
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
    model_override: Option<String>,
    effort_override: Option<String>,
) -> Result<(), String> {
    // Parse session DB id
    let session_db_id: i64 = session_id
        .parse()
        .map_err(|_| "Invalid session ID".to_string())?;

    let (worktree_path, stage, cli_session_id, session_provider, session_model) = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        let mut stmt = db
            .prepare("SELECT worktree_path, stage, session_id, provider, model FROM sessions WHERE id = ?1")
            .map_err(|e| format!("Query error: {e}"))?;

        stmt.query_row(rusqlite::params![session_db_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3).unwrap_or_else(|_| "claude".to_string()),
                row.get::<_, String>(4).unwrap_or_else(|_| "claude-sonnet-4-6".to_string()),
            ))
        })
        .map_err(|e| format!("Session not found: {e}"))?
    };

    let worktree_path = worktree_path.ok_or("No worktree for this session")?;

    // Use session's model by default, allow override
    let effective_model = model_override.unwrap_or(session_model);
    let effective_provider = provider::provider_for_model(&effective_model)
        .unwrap_or_else(|| {
            provider::ProviderKind::from_str(&session_provider)
                .unwrap_or(provider::ProviderKind::Claude)
        });

    let the_provider = provider::get_provider_by_kind(effective_provider);
    the_provider.find_binary()?;

    let (prompt, stage_effort) = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        match db::get_prompt(&db, &stage)? {
            Some(p) => (p.prompt_text, p.effort),
            None => (String::new(), "high".to_string()),
        }
    };

    let effective_effort = effort_override.unwrap_or(stage_effort);

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

    // Emit the user's message to the log
    emit_user_message(&app_handle, session_db_id, &message);

    // Enable sleep prevention if this is the first active session
    let sleep_enabled = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::get_app_settings(&db)
            .map(|s| s.sleep_prevention)
            .unwrap_or(true)
    };
    crate::sleep::on_session_start(sleep_enabled).await;

    // Get repo_id and issue_number for signal handling
    let (respond_repo_id, respond_issue_number) = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        let mut stmt = db
            .prepare("SELECT repo_id, issue_number FROM sessions WHERE id = ?1")
            .map_err(|e| format!("Query error: {e}"))?;
        stmt.query_row(rusqlite::params![session_db_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|e| format!("Session not found: {e}"))?
    };

    let app = app_handle.clone();
    let wt_path = worktree_path;
    let provider_kind = effective_provider;
    let model_for_spawn = effective_model;
    let effort_for_spawn = effective_effort;
    let mcp_binary = provider::find_mcp_binary();
    let respond_stage = stage.clone();

    tokio::spawn(async move {
        let prov = provider::get_provider_by_kind(provider_kind);
        let config = SessionConfig {
            worktree_path: wt_path,
            system_prompt: prompt,
            user_prompt: message,
            permission_mode,
            model: model_for_spawn,
            effort: effort_for_spawn,
            resume_session_id: cli_session_id,
            mcp_binary_path: mcp_binary,
        };

        let result = provider::run_provider_session(prov.as_ref(), config, session_db_id, &app).await;

        match result {
            Ok(res) => {
                if let Some(cost) = res.cost_usd {
                    save_session_cost(&app, session_db_id, cost);
                }

                // Handle signals based on what stage this session is in
                match res.signal {
                    Some(provider::StateSignal::AdvanceToInProgress) if respond_stage == "spec" => {
                        // Spec complete after blocked response — auto-start implement
                        update_status_via_app(&app, session_db_id, "completed", None);
                        set_issue_column_via_app(&app, respond_repo_id, respond_issue_number, "in_progress");
                        auto_start_implement(&app, respond_repo_id, respond_issue_number).await;
                    }
                    Some(provider::StateSignal::AdvanceToReview) if respond_stage == "implement" => {
                        // Implement complete after blocked response
                        update_status_via_app(&app, session_db_id, "completed", None);
                        set_issue_column_via_app(&app, respond_repo_id, respond_issue_number, "review");
                    }
                    Some(provider::StateSignal::AdvanceToBlocked) => {
                        // Still blocked — need more input
                        update_status_via_app(&app, session_db_id, "completed", None);
                        set_issue_column_via_app(&app, respond_repo_id, respond_issue_number, "blocked");
                    }
                    _ => {
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
        "spec" => session_start(state, app_handle, repo_id, issue_number, None, None, None).await,
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
        if pid > 1 {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGTERM);
            }
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
    provider: String,
    model: String,
    effort: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    db::update_prompt(&db, &stage, &prompt_text, &provider, &model, &effort)
}

#[tauri::command]
pub async fn prompts_reset(
    state: tauri::State<'_, AppState>,
    stage: String,
) -> Result<AgentPrompt, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    db::reset_prompt(&db, &stage)
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

// ── State Advancement Helpers ───────────────────────────────────────────

/// Update issue column in DB and emit event to frontend.
fn set_issue_column_via_app(app: &tauri::AppHandle, repo_id: i64, issue_number: i64, column: &str) {
    let state = app.state::<AppState>();
    if let Ok(db) = state.db.lock() {
        let _ = db::set_issue_column(&db, repo_id, issue_number, column);
    }
    let _ = app.emit(
        "issue-column-changed",
        serde_json::json!({
            "repo_id": repo_id,
            "issue_number": issue_number,
            "column_id": column,
        }),
    );
}

/// Auto-start the implement phase for an issue (called after spec signals advance_to_in_progress).
async fn auto_start_implement(app: &tauri::AppHandle, repo_id: i64, issue_number: i64) {
    // Check for duplicate active sessions
    {
        let state = app.state::<AppState>();
        let Ok(db) = state.db.lock() else { return };
        if let Ok(Some(_)) = db::get_active_session(&db, repo_id, issue_number) {
            return; // Already has an active session
        }
    }

    let (implement_prompt, stage_model, stage_effort) = {
        let state = app.state::<AppState>();
        let Ok(db) = state.db.lock() else { return };
        match db::get_prompt(&db, "implement") {
            Ok(Some(p)) => (p.prompt_text, p.model, p.effort),
            _ => (
                "Implement the feature as specified.".to_string(),
                "claude-sonnet-4-6".to_string(),
                "high".to_string(),
            ),
        }
    };

    let effective_provider = provider::provider_for_model(&stage_model)
        .unwrap_or(provider::ProviderKind::Claude);
    let the_provider = provider::get_provider_by_kind(effective_provider);
    if the_provider.find_binary().is_err() {
        return;
    }

    let worktree_path = {
        let state = app.state::<AppState>();
        let Ok(db) = state.db.lock() else { return };
        match db::get_latest_session(&db, repo_id, issue_number) {
            Ok(Some(s)) => s.worktree_path,
            _ => None,
        }
    };
    let Some(worktree_path) = worktree_path else { return };

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
        provider: effective_provider.as_str().to_string(),
        model: stage_model.clone(),
    };

    let session_db_id = {
        let state = app.state::<AppState>();
        let Ok(db) = state.db.lock() else { return };
        match db::insert_session(&db, &session) {
            Ok(id) => id,
            Err(_) => return,
        }
    };

    let session = Session {
        id: session_db_id.to_string(),
        ..session
    };
    let _ = app.emit("session-status", &session);

    let (sleep_enabled, permission_mode) = {
        let state = app.state::<AppState>();
        let Ok(db) = state.db.lock() else { return };
        match db::get_app_settings(&db) {
            Ok(settings) => {
                let mode = if settings.bypass_permissions {
                    "bypassPermissions".to_string()
                } else {
                    "auto".to_string()
                };
                (settings.sleep_prevention, mode)
            }
            Err(_) => (true, "auto".to_string()),
        }
    };
    crate::sleep::on_session_start(sleep_enabled).await;

    let user_prompt = format!(
        "GitHub Issue #{issue_number}\n\nImplement the feature described in the issue and spec. Write clean, well-tested code."
    );
    let mcp_binary = provider::find_mcp_binary();

    let app2 = app.clone();
    tokio::spawn(async move {
        let prov = provider::get_provider_by_kind(effective_provider);
        let config = provider::SessionConfig {
            worktree_path,
            system_prompt: implement_prompt,
            user_prompt,
            permission_mode,
            model: stage_model,
            effort: stage_effort,
            resume_session_id: None,
            mcp_binary_path: mcp_binary,
        };

        let result = provider::run_provider_session(prov.as_ref(), config, session_db_id, &app2).await;
        handle_implement_result(&app2, result, session_db_id, repo_id, issue_number).await;
        crate::sleep::on_session_end().await;
    });
}

/// Handle the result of an implement session (shared by session_start_implement and auto_start_implement).
async fn handle_implement_result(
    app: &tauri::AppHandle,
    result: Result<provider::ProviderSessionResult, String>,
    session_db_id: i64,
    repo_id: i64,
    issue_number: i64,
) {
    match result {
        Ok(res) => {
            if let Some(cost) = res.cost_usd {
                save_session_cost(app, session_db_id, cost);
            }

            match res.signal {
                Some(provider::StateSignal::AdvanceToReview) => {
                    update_status_via_app(app, session_db_id, "completed", None);
                    set_issue_column_via_app(app, repo_id, issue_number, "review");
                    let _ = app.emit(
                        "session-log",
                        SessionLogEvent {
                            session_id: session_db_id.to_string(),
                            entry: SessionLogEntry {
                                id: uuid::Uuid::new_v4().to_string(),
                                session_id: session_db_id.to_string(),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                event_type: "status_change".to_string(),
                                content: "Implementation complete — moved to review".to_string(),
                            },
                        },
                    );
                }
                Some(provider::StateSignal::AdvanceToBlocked) => {
                    update_status_via_app(app, session_db_id, "completed", None);
                    set_issue_column_via_app(app, repo_id, issue_number, "blocked");
                }
                _ => {
                    // No signal — just mark completed and advance to review by default
                    update_status_via_app(app, session_db_id, "completed", None);
                    set_issue_column_via_app(app, repo_id, issue_number, "review");
                    let _ = app.emit(
                        "session-advance",
                        serde_json::json!({
                            "session_id": session_db_id.to_string(),
                            "next_stage": "review",
                        }),
                    );
                }
            }
        }
        Err(e) => {
            update_status_via_app(app, session_db_id, "failed", Some(&e));
            let _ = app.emit(
                "session-error",
                serde_json::json!({
                    "session_id": session_db_id.to_string(),
                    "error": e,
                }),
            );
        }
    }
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

/// Save the session cost to the database.
fn save_session_cost(app: &tauri::AppHandle, session_db_id: i64, cost: f64) {
    let state = app.state::<AppState>();
    let Ok(db) = state.db.lock() else { return };
    let _ = db.execute(
        "UPDATE sessions SET cost_usd = ?1 WHERE id = ?2",
        rusqlite::params![cost, session_db_id],
    );
}

/// Emit a user message to the session log (both DB and frontend).
fn emit_user_message(app: &tauri::AppHandle, session_db_id: i64, message: &str) {
    let state = app.state::<AppState>();
    if let Ok(db) = state.db.lock() {
        let _ = db::insert_session_log(&db, session_db_id, "user_message", message);
    }
    let _ = app.emit(
        "session-log",
        SessionLogEvent {
            session_id: session_db_id.to_string(),
            entry: SessionLogEntry {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: session_db_id.to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                event_type: "user_message".to_string(),
                content: message.to_string(),
            },
        },
    );
}

// ── Models Command ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_models() -> Result<serde_json::Value, String> {
    let models: Vec<serde_json::Value> = provider::all_models()
        .iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id,
                "display_name": m.display_name,
                "provider": m.provider.as_str(),
                "default_effort": m.default_effort,
                "effort_levels": m.effort_levels,
            })
        })
        .collect();
    Ok(serde_json::json!(models))
}
