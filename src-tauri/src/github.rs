use reqwest::Client;
use serde_json::{json, Value};

use crate::db;
use crate::types::*;
use crate::AppState;

const GITHUB_API: &str = "https://api.github.com";

fn github_headers(token: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    headers.insert(
        reqwest::header::ACCEPT,
        "application/vnd.github+json".parse().unwrap(),
    );
    headers.insert(
        reqwest::header::USER_AGENT,
        "AutoDev".parse().unwrap(),
    );
    headers
}

// ── Auth Commands ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn github_auth_from_cli(
    state: tauri::State<'_, AppState>,
) -> Result<GitHubUser, String> {
    // Get token from gh CLI
    let output = tokio::process::Command::new("gh")
        .args(["auth", "token"])
        .output()
        .await
        .map_err(|e| format!("Failed to run `gh auth token`. Is the GitHub CLI installed?\n{e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Not authenticated with gh CLI. Run `gh auth login` first.\n{stderr}"
        ));
    }

    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        return Err("gh auth token returned empty. Run `gh auth login` first.".to_string());
    }

    // Validate the token by fetching the current user
    let user = fetch_current_user(&state.http_client, &token).await?;

    // Save to DB
    let db = state.db.lock().map_err(|e| format!("DB lock error: {e}"))?;
    db::save_auth_token(&db, &token, &user.login)?;

    Ok(user)
}

#[tauri::command]
pub async fn github_get_auth_status(
    state: tauri::State<'_, AppState>,
) -> Result<AuthStatusResponse, String> {
    let token_info = {
        let db = state.db.lock().map_err(|e| format!("DB lock error: {e}"))?;
        db::get_auth_token(&db)?
    };

    match token_info {
        Some((token, _username)) => {
            match fetch_current_user(&state.http_client, &token).await {
                Ok(user) => Ok(AuthStatusResponse {
                    authenticated: true,
                    user: Some(user),
                }),
                Err(_) => {
                    // Token might be expired, clear it
                    let db = state.db.lock().map_err(|e| format!("DB lock error: {e}"))?;
                    db::delete_auth(&db)?;
                    Ok(AuthStatusResponse {
                        authenticated: false,
                        user: None,
                    })
                }
            }
        }
        None => Ok(AuthStatusResponse {
            authenticated: false,
            user: None,
        }),
    }
}

#[tauri::command]
pub async fn github_logout(state: tauri::State<'_, AppState>) -> Result<(), String> {
    crate::polling::stop_polling();
    let db = state.db.lock().map_err(|e| format!("DB lock error: {e}"))?;
    db::delete_auth(&db)
}

// ── Repo Commands ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn github_list_user_repos(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<GitHubRepo>, String> {
    let token = get_token(&state)?;
    let client = &state.http_client;

    let mut all_repos = Vec::new();
    let mut page = 1u32;

    loop {
        let resp = client
            .get(format!(
                "{GITHUB_API}/user/repos?per_page=100&sort=updated&affiliation=owner,collaborator,organization_member&page={page}"
            ))
            .headers(github_headers(&token))
            .send()
            .await
            .map_err(|e| format!("Failed to fetch repos: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("GitHub API error: {}", resp.status()));
        }

        let repos: Vec<GitHubRepo> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse repos: {e}"))?;

        let count = repos.len();
        all_repos.extend(repos);

        if count < 100 {
            break;
        }
        page += 1;
    }

    Ok(all_repos)
}

#[tauri::command]
pub async fn github_add_repo(
    state: tauri::State<'_, AppState>,
    owner: String,
    name: String,
) -> Result<RepoConfig, String> {
    let token = get_token(&state)?;
    let client = &state.http_client;

    // Fetch repo info from GitHub
    let resp = client
        .get(format!("{GITHUB_API}/repos/{owner}/{name}"))
        .headers(github_headers(&token))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch repo: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "GitHub API error {}: could not find {owner}/{name}",
            resp.status()
        ));
    }

    let gh_repo: GitHubRepo = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse repo response: {e}"))?;

    let base_branch = gh_repo.default_branch.unwrap_or_else(|| "main".to_string());

    let repo = RepoConfig {
        id: 0, // will be set by DB
        github_id: gh_repo.id,
        owner: owner.clone(),
        name: name.clone(),
        full_name: format!("{owner}/{name}"),
        setup_script: String::new(),
        run_script: String::new(),
        base_branch,
        branch_prefix: "autodev/".to_string(),
        worktree_dir: ".worktrees/".to_string(),
    };

    let repo_id = {
        let db = state.db.lock().map_err(|e| format!("DB lock error: {e}"))?;
        db::insert_repo(&db, &repo)?
    };

    // Ensure autodev labels exist on the repo
    ensure_labels(client, &token, &owner, &name).await?;

    Ok(RepoConfig { id: repo_id, ..repo })
}

#[tauri::command]
pub async fn github_remove_repo(
    state: tauri::State<'_, AppState>,
    repo_id: i64,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| format!("DB lock error: {e}"))?;
    db::delete_repo(&db, repo_id)
}

#[tauri::command]
pub async fn github_get_repos(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<RepoConfig>, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock error: {e}"))?;
    db::get_all_repos(&db)
}

#[tauri::command]
pub async fn github_update_repo(
    state: tauri::State<'_, AppState>,
    repo: RepoConfig,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| format!("DB lock error: {e}"))?;
    db::update_repo(&db, &repo)
}

// ── Collaborator Commands ────────────────────────────────────────────────

#[tauri::command]
pub async fn github_list_collaborators(
    state: tauri::State<'_, AppState>,
    owner: String,
    name: String,
) -> Result<Vec<GitHubUser>, String> {
    let token = get_token(&state)?;
    let client = &state.http_client;

    let resp = client
        .get(format!(
            "{GITHUB_API}/repos/{owner}/{name}/collaborators?per_page=100"
        ))
        .headers(github_headers(&token))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch collaborators: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API error {}", resp.status()));
    }

    let users: Vec<GitHubUser> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse collaborators: {e}"))?;

    Ok(users)
}

// ── Issue Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn github_fetch_issues(
    state: tauri::State<'_, AppState>,
    owner: String,
    name: String,
) -> Result<Vec<Issue>, String> {
    let token = get_token(&state)?;
    let client = &state.http_client;

    let resp = client
        .get(format!(
            "{GITHUB_API}/repos/{owner}/{name}/issues?state=all&per_page=100&sort=updated"
        ))
        .headers(github_headers(&token))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch issues: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API error {}", resp.status()));
    }

    let mut issues: Vec<Issue> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse issues: {e}"))?;

    // Annotate with repo info
    for issue in &mut issues {
        issue.repo_owner = owner.clone();
        issue.repo_name = name.clone();
    }

    Ok(issues)
}

#[tauri::command]
pub async fn github_create_issue(
    state: tauri::State<'_, AppState>,
    owner: String,
    name: String,
    title: String,
    body: String,
    assignee: Option<String>,
) -> Result<Issue, String> {
    let token = get_token(&state)?;
    let client = &state.http_client;

    let mut payload = json!({
        "title": title,
        "body": body,
    });

    if let Some(ref a) = assignee {
        payload["assignees"] = json!([a]);
    }

    let resp = client
        .post(format!("{GITHUB_API}/repos/{owner}/{name}/issues"))
        .headers(github_headers(&token))
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to create issue: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error {status}: {text}"));
    }

    let mut issue: Issue = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse created issue: {e}"))?;

    issue.repo_owner = owner;
    issue.repo_name = name;

    Ok(issue)
}

#[tauri::command]
pub async fn github_add_label(
    state: tauri::State<'_, AppState>,
    owner: String,
    name: String,
    issue_number: i64,
    label: String,
) -> Result<(), String> {
    let token = get_token(&state)?;
    let client = &state.http_client;

    let resp = client
        .post(format!(
            "{GITHUB_API}/repos/{owner}/{name}/issues/{issue_number}/labels"
        ))
        .headers(github_headers(&token))
        .json(&json!({ "labels": [label] }))
        .send()
        .await
        .map_err(|e| format!("Failed to add label: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API error adding label: {}", resp.status()));
    }

    Ok(())
}

#[tauri::command]
pub async fn github_remove_label(
    state: tauri::State<'_, AppState>,
    owner: String,
    name: String,
    issue_number: i64,
    label: String,
) -> Result<(), String> {
    let token = get_token(&state)?;
    let client = &state.http_client;

    let encoded_label = urlencoding_label(&label);
    let resp = client
        .delete(format!(
            "{GITHUB_API}/repos/{owner}/{name}/issues/{issue_number}/labels/{encoded_label}"
        ))
        .headers(github_headers(&token))
        .send()
        .await
        .map_err(|e| format!("Failed to remove label: {e}"))?;

    // Ignore 404 — label might not be present
    if !resp.status().is_success() && resp.status() != reqwest::StatusCode::NOT_FOUND {
        return Err(format!(
            "GitHub API error removing label: {}",
            resp.status()
        ));
    }

    Ok(())
}

#[tauri::command]
pub async fn github_post_comment(
    state: tauri::State<'_, AppState>,
    owner: String,
    name: String,
    issue_number: i64,
    body: String,
) -> Result<(), String> {
    let token = get_token(&state)?;
    let client = &state.http_client;

    let resp = client
        .post(format!(
            "{GITHUB_API}/repos/{owner}/{name}/issues/{issue_number}/comments"
        ))
        .headers(github_headers(&token))
        .json(&json!({ "body": body }))
        .send()
        .await
        .map_err(|e| format!("Failed to post comment: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "GitHub API error posting comment: {}",
            resp.status()
        ));
    }

    Ok(())
}

#[tauri::command]
pub async fn github_create_pr(
    state: tauri::State<'_, AppState>,
    owner: String,
    name: String,
    title: String,
    body: String,
    head: String,
    base: String,
) -> Result<Value, String> {
    let token = get_token(&state)?;
    let client = &state.http_client;

    let resp = client
        .post(format!("{GITHUB_API}/repos/{owner}/{name}/pulls"))
        .headers(github_headers(&token))
        .json(&json!({
            "title": title,
            "body": body,
            "head": head,
            "base": base,
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to create PR: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error creating PR {status}: {text}"));
    }

    let pr: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse PR response: {e}"))?;

    Ok(pr)
}

#[tauri::command]
pub async fn github_squash_merge(
    state: tauri::State<'_, AppState>,
    owner: String,
    name: String,
    pull_number: i64,
) -> Result<(), String> {
    let token = get_token(&state)?;
    let client = &state.http_client;

    let resp = client
        .put(format!(
            "{GITHUB_API}/repos/{owner}/{name}/pulls/{pull_number}/merge"
        ))
        .headers(github_headers(&token))
        .json(&json!({ "merge_method": "squash" }))
        .send()
        .await
        .map_err(|e| format!("Failed to merge PR: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error merging PR {status}: {text}"));
    }

    Ok(())
}

#[tauri::command]
pub async fn github_close_issue(
    state: tauri::State<'_, AppState>,
    owner: String,
    name: String,
    issue_number: i64,
) -> Result<(), String> {
    let token = get_token(&state)?;
    let client = &state.http_client;

    let resp = client
        .patch(format!(
            "{GITHUB_API}/repos/{owner}/{name}/issues/{issue_number}"
        ))
        .headers(github_headers(&token))
        .json(&json!({ "state": "closed" }))
        .send()
        .await
        .map_err(|e| format!("Failed to close issue: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "GitHub API error closing issue: {}",
            resp.status()
        ));
    }

    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn get_token(state: &tauri::State<'_, AppState>) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock error: {e}"))?;
    let auth = db::get_auth_token(&db)?;
    auth.map(|(token, _)| token)
        .ok_or_else(|| "Not authenticated. Please log in with GitHub first.".to_string())
}

async fn fetch_current_user(client: &Client, token: &str) -> Result<GitHubUser, String> {
    let resp = client
        .get(format!("{GITHUB_API}/user"))
        .headers(github_headers(token))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch user: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API error: {}", resp.status()));
    }

    let user: GitHubUser = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse user: {e}"))?;

    Ok(user)
}

async fn ensure_labels(
    client: &Client,
    token: &str,
    owner: &str,
    name: &str,
) -> Result<(), String> {
    let labels = [
        ("autodev:claimed", "0E8A16"),
        ("autodev:in-progress", "1D76DB"),
        ("autodev:blocked", "E4E669"),
        ("autodev:review", "5319E7"),
    ];

    for (label_name, color) in &labels {
        let resp = client
            .post(format!("{GITHUB_API}/repos/{owner}/{name}/labels"))
            .headers(github_headers(token))
            .json(&json!({
                "name": label_name,
                "color": color,
            }))
            .send()
            .await;

        match resp {
            Ok(r) => {
                // 422 means label already exists — that's fine
                if !r.status().is_success()
                    && r.status() != reqwest::StatusCode::UNPROCESSABLE_ENTITY
                {
                    eprintln!("Warning: Failed to create label {label_name}: {}", r.status());
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to create label {label_name}: {e}");
            }
        }
    }

    Ok(())
}

/// Percent-encode a label for use in URL paths (colons are fine per GitHub API)
fn urlencoding_label(label: &str) -> String {
    label.replace(' ', "%20")
}

/// Fetch issues for a repo (non-command helper used by polling)
pub async fn fetch_issues_for_repo(
    client: &Client,
    token: &str,
    owner: &str,
    name: &str,
) -> Result<Vec<Issue>, String> {
    let resp = client
        .get(format!(
            "{GITHUB_API}/repos/{owner}/{name}/issues?state=all&per_page=100&sort=updated"
        ))
        .headers(github_headers(token))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch issues: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API error {}", resp.status()));
    }

    let mut issues: Vec<Issue> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse issues: {e}"))?;

    for issue in &mut issues {
        issue.repo_owner = owner.to_string();
        issue.repo_name = name.to_string();
    }

    Ok(issues)
}
