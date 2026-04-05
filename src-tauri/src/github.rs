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
    };

    let repo_id = {
        let db = state.db.lock().map_err(|e| format!("DB lock error: {e}"))?;
        db::insert_repo(&db, &repo)?
    };

    // Clone the repo to ~/.autodev/{name}/ if not already there
    let home = std::env::var("HOME")
        .map_err(|_| "HOME environment variable not set".to_string())?;
    let repo_dir = std::path::Path::new(&home)
        .join(".autodev")
        .join(&name);

    if !repo_dir.join(".git").exists() {
        std::fs::create_dir_all(&repo_dir)
            .map_err(|e| format!("Failed to create repo directory: {e}"))?;

        let clone_output = tokio::process::Command::new("gh")
            .args(["repo", "clone", &format!("{owner}/{name}"), repo_dir.to_str().unwrap()])
            .output()
            .await
            .map_err(|e| format!("Failed to clone repo: {e}"))?;

        if !clone_output.status.success() {
            let stderr = String::from_utf8_lossy(&clone_output.stderr);
            // Clean up the directory on failure
            let _ = std::fs::remove_dir_all(&repo_dir);
            return Err(format!("Failed to clone {owner}/{name}: {stderr}"));
        }
    }

    // Store the local path
    {
        let db = state.db.lock().map_err(|e| format!("DB lock error: {e}"))?;
        db::set_setting(&db, &format!("repo_{repo_id}_path"), repo_dir.to_str().unwrap())?;
    }

    Ok(RepoConfig { id: repo_id, ..repo })
}

#[tauri::command]
pub async fn github_add_local_repo(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<RepoConfig, String> {
    // Validate .git directory exists
    let git_dir = std::path::Path::new(&path).join(".git");
    if !git_dir.exists() {
        return Err("Selected folder is not a git repository (no .git directory found).".to_string());
    }

    // Get remote URL
    let remote_output = tokio::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(&path)
        .output()
        .await
        .map_err(|e| format!("Failed to read git remote: {e}"))?;

    if !remote_output.status.success() {
        return Err("No 'origin' remote found. The repository must have a GitHub remote.".to_string());
    }

    let remote_url = String::from_utf8_lossy(&remote_output.stdout).trim().to_string();
    let (owner, name) = parse_github_remote(&remote_url)
        .ok_or_else(|| format!("Could not parse GitHub owner/name from remote URL: {remote_url}"))?;

    // Fetch repo info from GitHub
    let token = get_token(&state)?;
    let client = &state.http_client;

    let resp = client
        .get(format!("{GITHUB_API}/repos/{owner}/{name}"))
        .headers(github_headers(&token))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch repo from GitHub: {e}"))?;

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
        id: 0,
        github_id: gh_repo.id,
        owner: owner.clone(),
        name: name.clone(),
        full_name: format!("{owner}/{name}"),
        setup_script: String::new(),
        run_script: String::new(),
        base_branch,
        branch_prefix: "autodev/".to_string(),
    };

    let repo_id = {
        let db = state.db.lock().map_err(|e| format!("DB lock error: {e}"))?;
        db::insert_repo(&db, &repo)?
    };

    // Store the local path
    {
        let db = state.db.lock().map_err(|e| format!("DB lock error: {e}"))?;
        db::set_setting(&db, &format!("repo_{repo_id}_path"), &path)?;
    }

    Ok(RepoConfig { id: repo_id, ..repo })
}

/// Parse a GitHub remote URL into (owner, name).
/// Supports HTTPS and SSH formats.
fn parse_github_remote(url: &str) -> Option<(String, String)> {
    // SSH: git@github.com:owner/repo.git
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        let rest = rest.strip_suffix(".git").unwrap_or(rest);
        let parts: Vec<&str> = rest.splitn(2, '/').collect();
        if parts.len() == 2 {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }

    // HTTPS: https://github.com/owner/repo.git
    if url.contains("github.com") {
        let url = url.strip_suffix(".git").unwrap_or(url);
        let parts: Vec<&str> = url.rsplitn(3, '/').collect();
        if parts.len() >= 2 {
            return Some((parts[1].to_string(), parts[0].to_string()));
        }
    }

    None
}

#[tauri::command]
pub async fn github_get_repo_removal_info(
    state: tauri::State<'_, AppState>,
    repo_id: i64,
) -> Result<RepoRemovalInfo, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock error: {e}"))?;

    let repo = db::get_repo_by_id(&db, repo_id)?
        .ok_or_else(|| format!("Repo {repo_id} not found"))?;

    let local_path = db::get_setting(&db, &format!("repo_{repo_id}_path"))?;
    let worktree_paths = db::get_worktree_paths_for_repo(&db, repo_id)?;
    let session_count = db::count_sessions_for_repo(&db, repo_id)?;
    let log_count = db::count_session_logs_for_repo(&db, repo_id)?;

    Ok(RepoRemovalInfo {
        repo_name: repo.full_name,
        local_path,
        worktree_paths,
        session_count,
        log_count,
    })
}

#[tauri::command]
pub async fn github_remove_repo(
    state: tauri::State<'_, AppState>,
    repo_id: i64,
) -> Result<(), String> {
    // Gather info before deleting
    let (repo, repo_path, worktree_paths) = {
        let db = state.db.lock().map_err(|e| format!("DB lock error: {e}"))?;
        let repo = db::get_repo_by_id(&db, repo_id)?
            .ok_or_else(|| format!("Repo {repo_id} not found"))?;
        let repo_path = db::get_setting(&db, &format!("repo_{repo_id}_path"))?;
        let worktree_paths = db::get_worktree_paths_for_repo(&db, repo_id)?;
        (repo, repo_path, worktree_paths)
    };

    // Clean up worktrees on disk
    if let Some(ref rp) = repo_path {
        for wt_path in &worktree_paths {
            // Extract issue number from worktree path to build branch name
            let slug = std::path::Path::new(wt_path)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("");
            let branch_name = format!("{}{}", repo.branch_prefix, slug);

            if let Err(e) = crate::worktrees::remove_worktree(rp, wt_path, &branch_name).await {
                eprintln!("Warning: Failed to clean up worktree {wt_path}: {e}");
            }
        }
    }

    // Cascade delete all DB records
    let db = state.db.lock().map_err(|e| format!("DB lock error: {e}"))?;
    db::delete_repo_cascade(&db, repo_id)
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
