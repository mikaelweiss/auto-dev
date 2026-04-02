use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GitHubUser {
    pub login: String,
    pub avatar_url: String,
    pub id: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GitHubLabel {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PullRequestRef {
    pub url: String,
    pub html_url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Issue {
    pub id: i64,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub assignee: Option<GitHubUser>,
    pub labels: Vec<GitHubLabel>,
    pub created_at: String,
    pub updated_at: String,
    pub pull_request: Option<PullRequestRef>,
    pub html_url: String,
    #[serde(default)]
    pub repo_owner: String,
    #[serde(default)]
    pub repo_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Session {
    pub id: String,
    pub repo_id: i64,
    pub issue_number: i64,
    pub stage: String,
    pub worktree_path: Option<String>,
    pub session_id: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionLogEntry {
    pub id: String,
    pub session_id: String,
    pub timestamp: String,
    pub event_type: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RepoConfig {
    pub id: i64,
    pub github_id: i64,
    pub owner: String,
    pub name: String,
    pub full_name: String,
    pub setup_script: String,
    pub run_script: String,
    pub base_branch: String,
    pub branch_prefix: String,
    pub worktree_dir: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentPrompt {
    pub stage: String,
    pub prompt_text: String,
    pub is_default: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub sleep_prevention: bool,
    pub notifications_enabled: bool,
    pub poll_interval_seconds: i64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            sleep_prevention: true,
            notifications_enabled: true,
            poll_interval_seconds: 15,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthStatusResponse {
    pub authenticated: bool,
    pub user: Option<GitHubUser>,
}

/// Represents a GitHub repo as returned by the API
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GitHubRepo {
    pub id: i64,
    pub full_name: String,
    pub owner: GitHubRepoOwner,
    pub name: String,
    pub default_branch: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GitHubRepoOwner {
    pub login: String,
}

/// Information about what will be deleted when removing a repo
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RepoRemovalInfo {
    pub repo_name: String,
    pub local_path: Option<String>,
    pub worktree_paths: Vec<String>,
    pub session_count: i64,
    pub log_count: i64,
}

/// Events emitted to the frontend
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IssuesUpdatedEvent {
    pub issues: Vec<Issue>,
    pub repo_owner: String,
    pub repo_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionLogEvent {
    pub session_id: String,
    pub entry: SessionLogEntry,
}
