use rusqlite::{Connection, params};

use crate::types::{AgentPrompt, AppSettings, RepoConfig, Session, SessionLogEntry};

/// Get the database path (~/.autodev/autodev.db), creating the directory if needed.
pub fn db_path() -> Result<std::path::PathBuf, String> {
    let home = dirs_fallback()?;
    let dir = home.join(".autodev");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create ~/.autodev: {e}"))?;
    Ok(dir.join("autodev.db"))
}

fn dirs_fallback() -> Result<std::path::PathBuf, String> {
    std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .map_err(|_| "HOME environment variable not set".to_string())
}

/// Open (or create) the database and run migrations.
pub fn open_and_init() -> Result<Connection, String> {
    let path = db_path()?;
    let conn = Connection::open(&path).map_err(|e| format!("Failed to open DB: {e}"))?;

    // Enable WAL mode for better concurrency
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(|e| format!("Failed to set WAL mode: {e}"))?;

    create_tables(&conn)?;
    run_migrations(&conn)?;
    seed_default_prompts(&conn)?;
    Ok(conn)
}

fn create_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS repos (
            id INTEGER PRIMARY KEY,
            github_id INTEGER,
            owner TEXT NOT NULL,
            name TEXT NOT NULL,
            setup_script TEXT NOT NULL DEFAULT '',
            run_script TEXT NOT NULL DEFAULT '',
            base_branch TEXT NOT NULL DEFAULT 'main',
            branch_prefix TEXT NOT NULL DEFAULT 'autodev/',
            added_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY,
            repo_id INTEGER NOT NULL REFERENCES repos(id),
            issue_number INTEGER NOT NULL,
            stage TEXT NOT NULL,
            worktree_path TEXT,
            session_id TEXT,
            status TEXT NOT NULL DEFAULT 'running',
            error_message TEXT,
            started_at TEXT NOT NULL,
            completed_at TEXT
        );

        CREATE TABLE IF NOT EXISTS auth (
            id INTEGER PRIMARY KEY,
            provider TEXT NOT NULL,
            token TEXT NOT NULL,
            username TEXT,
            expires_at TEXT
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS agent_prompts (
            id INTEGER PRIMARY KEY,
            stage TEXT NOT NULL UNIQUE,
            prompt_text TEXT NOT NULL,
            is_default INTEGER NOT NULL DEFAULT 1,
            model TEXT NOT NULL DEFAULT 'haiku',
            effort TEXT NOT NULL DEFAULT 'high'
        );

        CREATE TABLE IF NOT EXISTS session_logs (
            id INTEGER PRIMARY KEY,
            session_id INTEGER NOT NULL REFERENCES sessions(id),
            timestamp TEXT NOT NULL,
            event_type TEXT NOT NULL,
            content TEXT NOT NULL
        );
        ",
    )
    .map_err(|e| format!("Failed to create tables: {e}"))
}

fn run_migrations(conn: &Connection) -> Result<(), String> {
    // Migration: relax CHECK constraints on sessions.status to allow 'initializing' and 'setup'.
    // SQLite doesn't support ALTER CONSTRAINT, so we recreate the table.
    // Only runs if the old constraint exists.
    let needs_migration: bool = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='sessions'",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .unwrap_or(None)
        .map(|sql| sql.contains("CHECK(status IN ('running', 'completed', 'failed'))"))
        .unwrap_or(false);

    if needs_migration {
        eprintln!("[DB] Migrating sessions table: relaxing status CHECK constraint");
        conn.execute_batch(
            "
            CREATE TABLE sessions_new (
                id INTEGER PRIMARY KEY,
                repo_id INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
                issue_number INTEGER NOT NULL,
                stage TEXT NOT NULL,
                worktree_path TEXT,
                session_id TEXT,
                status TEXT NOT NULL DEFAULT 'running',
                error_message TEXT,
                started_at TEXT NOT NULL DEFAULT (datetime('now')),
                completed_at TEXT
            );
            INSERT INTO sessions_new (id, repo_id, issue_number, stage, worktree_path, session_id, status, error_message, started_at, completed_at)
                SELECT id, repo_id, issue_number, stage, worktree_path, session_id, status, error_message, started_at, completed_at FROM sessions;
            DROP TABLE sessions;
            ALTER TABLE sessions_new RENAME TO sessions;
            CREATE INDEX IF NOT EXISTS idx_sessions_repo_issue ON sessions(repo_id, issue_number);
            ",
        )
        .map_err(|e| format!("Failed to migrate sessions table: {e}"))?;
        eprintln!("[DB] Migration complete");
    }

    // Migration: recreate session_logs with INTEGER types and no CHECK constraint.
    // The old table used TEXT PRIMARY KEY for id (never auto-assigned) and TEXT session_id.
    let needs_logs_migration: bool = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='session_logs'",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .unwrap_or(None)
        .map(|sql| sql.contains("id TEXT PRIMARY KEY") || sql.contains("CHECK(event_type"))
        .unwrap_or(false);

    if needs_logs_migration {
        eprintln!("[DB] Migrating session_logs table: fixing column types");
        conn.execute_batch(
            "
            CREATE TABLE session_logs_new (
                id INTEGER PRIMARY KEY,
                session_id INTEGER NOT NULL REFERENCES sessions(id),
                timestamp TEXT NOT NULL,
                event_type TEXT NOT NULL,
                content TEXT NOT NULL
            );
            INSERT INTO session_logs_new (session_id, timestamp, event_type, content)
                SELECT CAST(session_id AS INTEGER), timestamp, event_type, content FROM session_logs;
            DROP TABLE session_logs;
            ALTER TABLE session_logs_new RENAME TO session_logs;
            CREATE INDEX IF NOT EXISTS idx_session_logs_session ON session_logs(session_id);
            ",
        )
        .map_err(|e| format!("Failed to migrate session_logs table: {e}"))?;
        eprintln!("[DB] session_logs migration complete");
    }

    // Migration: add hidden column to sessions
    let has_hidden: bool = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='sessions'",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .unwrap_or(None)
        .map(|sql| sql.contains("hidden"))
        .unwrap_or(false);

    if !has_hidden {
        eprintln!("[DB] Migrating sessions table: adding hidden column");
        conn.execute_batch(
            "ALTER TABLE sessions ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0;",
        )
        .map_err(|e| format!("Failed to add hidden column: {e}"))?;
        eprintln!("[DB] hidden column migration complete");
    }

    // Migration: add cost_usd column to sessions
    let has_cost: bool = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='sessions'",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .unwrap_or(None)
        .map(|sql| sql.contains("cost_usd"))
        .unwrap_or(false);

    if !has_cost {
        eprintln!("[DB] Migrating sessions table: adding cost_usd column");
        conn.execute_batch("ALTER TABLE sessions ADD COLUMN cost_usd REAL;")
            .map_err(|e| format!("Failed to add cost_usd column: {e}"))?;
        eprintln!("[DB] cost_usd column migration complete");
    }

    // Migration: add model and effort columns to agent_prompts
    let has_model: bool = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='agent_prompts'",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .unwrap_or(None)
        .map(|sql| sql.contains("model"))
        .unwrap_or(false);

    if !has_model {
        eprintln!("[DB] Migrating agent_prompts: adding model and effort columns");
        conn.execute_batch(
            "ALTER TABLE agent_prompts ADD COLUMN model TEXT NOT NULL DEFAULT 'haiku';\
             ALTER TABLE agent_prompts ADD COLUMN effort TEXT NOT NULL DEFAULT 'high';",
        )
        .map_err(|e| format!("Failed to add model/effort columns: {e}"))?;
        eprintln!("[DB] agent_prompts model/effort migration complete");
    }

    Ok(())
}

fn seed_default_prompts(conn: &Connection) -> Result<(), String> {
    let defaults: &[(&str, &str)] = &[
        (
            "spec",
            "You are an AI developer analyzing a GitHub issue to produce a specification.\n\
             You have access to the `gh` CLI for interacting with GitHub.\n\n\
             ## Process\n\
             1. Check for an existing spec by reading the issue's comments:\n\
             \x20  `gh api repos/{owner}/{repo}/issues/{number}/comments --jq '.[].body'`\n\
             \x20  Look for a comment that starts with \"## Spec\" or contains a specification.\n\
             2. **If an existing spec is found**: Present it to the user and ask if there's anything they'd like to update. If they confirm it's good, skip to step 6.\n\
             3. **If no spec exists**: Read the issue thoroughly and explore the codebase to understand the architecture, conventions, and relevant code paths.\n\
             4. If you have blocking questions that prevent you from writing the spec:\n\
             \x20  a. Post a comment on the issue with your questions: `gh issue comment {number} -R {owner}/{repo} --body \"## Questions\\n\\n...\"`\n\
             \x20  b. Add the blocked label: `gh issue edit {number} -R {owner}/{repo} --add-label \"autodev:blocked\"`\n\
             \x20  c. Remove the planning label if present: `gh issue edit {number} -R {owner}/{repo} --remove-label \"autodev:planning\"`\n\
             \x20  d. Stop and tell the user you've posted questions on the issue.\n\
             5. Write the spec and post it as a comment on the issue:\n\
             \x20  `gh issue comment {number} -R {owner}/{repo} --body \"## Spec\\n\\n...\"`\n\
             6. Move the issue to in-progress to trigger implementation:\n\
             \x20  a. Add the in-progress label: `gh issue edit {number} -R {owner}/{repo} --add-label \"autodev:in-progress\"`\n\
             \x20  b. Remove planning/blocked labels if present: `gh issue edit {number} -R {owner}/{repo} --remove-label \"autodev:planning\" --remove-label \"autodev:blocked\"`\n\n\
             ## Specification format\n\
             Your spec comment should include:\n\
             - **Summary**: One-sentence description of what this change does.\n\
             - **Relevant files**: List every file you expect to touch, with a brief note on what changes.\n\
             - **Approach**: Step-by-step plan for the implementation. Be specific — reference functions, types, and modules by name.\n\
             - **Edge cases**: Anything that could go wrong or needs special handling.\n\n\
             ## Rules\n\
             - Do NOT make any code changes. This is a read-only analysis stage.\n\
             - Do NOT guess at implementation details you haven't verified by reading the code.\n\
             - Keep the spec concise and actionable — a developer should be able to implement from it.\n\
             - Always use the `gh` CLI to interact with GitHub — never modify labels or comments through any other means.",
        ),
        (
            "implement",
            "You are an AI developer implementing a feature or fix.\n\n\
             ## Process\n\
             1. Read the issue and any spec comments to understand exactly what to build.\n\
             2. Explore the codebase to understand existing patterns, conventions, and style.\n\
             3. Implement the changes. Follow the existing code style precisely — match naming, formatting, error handling, and patterns already in use.\n\
             4. Write tests for your changes if the project has a testing convention. Match the existing test style.\n\
             5. Run any existing tests or build commands to verify you haven't broken anything.\n\
             6. Commit your changes with a clear, concise commit message.\n\n\
             ## Rules\n\
             - Do the minimum necessary to solve the issue. Do not refactor unrelated code, add unnecessary abstractions, or over-engineer.\n\
             - Do not add comments, docstrings, or type annotations to code you didn't change.\n\
             - If the project has a CLAUDE.md or similar configuration, follow its instructions.\n\
             - If you're unsure about something, implement the simplest reasonable approach rather than guessing at complexity.",
        ),
        (
            "review",
            "You are an AI developer reviewing and polishing code before it becomes a PR.\n\n\
             ## Process\n\
             1. Review the diff carefully for:\n\
                - Bugs and logic errors\n\
                - Missing edge cases or error handling\n\
                - Style inconsistencies with the rest of the codebase\n\
                - Test coverage gaps\n\
                - Security issues (injection, XSS, leaked secrets, etc.)\n\
             2. Fix any issues you find directly — do not just comment on them.\n\
             3. Run tests to verify your fixes don't break anything.\n\
             4. Commit your fixes with clear commit messages.\n\n\
             ## Rules\n\
             - Only fix real problems. Do not nitpick style, add comments, or refactor working code.\n\
             - Do not rewrite the implementation. Fix bugs and gaps, preserve the author's approach.\n\
             - If the code is clean and correct, say so and move on.",
        ),
        (
            "ci_fix",
            "You are an AI developer fixing a CI failure.\n\n\
             ## Process\n\
             1. Read the CI failure output carefully. Identify the root cause — don't just fix the symptom.\n\
             2. Explore the relevant code to understand why the failure occurred.\n\
             3. Fix the underlying issue.\n\
             4. Run the failing tests or build locally to verify your fix works.\n\
             5. Commit your fix with a clear commit message referencing what was broken.\n\n\
             ## Rules\n\
             - Fix the root cause, not the symptom. Do not suppress warnings, skip tests, or add workarounds.\n\
             - Do not change test expectations to match broken behavior.\n\
             - If the CI failure reveals a real bug in the code, fix the code — not the test.\n\
             - Keep changes minimal — only fix what's broken.",
        ),
        (
            "merge_conflict",
            "You are an AI developer resolving merge conflicts.\n\n\
             ## Process\n\
             1. Understand the intent of BOTH sides — the PR changes and the base branch changes.\n\
             2. Resolve conflicts by preserving the intent of both sides. If they conflict semantically (not just textually), prefer the PR's intent but incorporate base branch changes.\n\
             3. Run tests after resolving to make sure nothing is broken.\n\
             4. Commit the resolution with a clear message.\n\n\
             ## Rules\n\
             - Never blindly accept one side. Always understand what both sides were trying to do.\n\
             - If the conflict is complex or ambiguous, resolve it conservatively and note what you chose and why.",
        ),
    ];

    for (stage, prompt) in defaults {
        conn.execute(
            "INSERT OR IGNORE INTO agent_prompts (stage, prompt_text, is_default) VALUES (?1, ?2, 1)",
            params![stage, prompt],
        )
        .map_err(|e| format!("Failed to seed prompt for {stage}: {e}"))?;
    }

    Ok(())
}

// ── Auth ────────────────────────────────────────────────────────────────

pub fn save_auth_token(conn: &Connection, token: &str, username: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO auth (id, provider, token, username) VALUES (1, 'github', ?1, ?2)",
        params![token, username],
    )
    .map_err(|e| format!("Failed to save auth: {e}"))?;
    Ok(())
}

pub fn get_auth_token(conn: &Connection) -> Result<Option<(String, String)>, String> {
    let mut stmt = conn
        .prepare("SELECT token, username FROM auth WHERE provider = 'github' LIMIT 1")
        .map_err(|e| format!("Failed to query auth: {e}"))?;

    let result = stmt
        .query_row([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
            ))
        });

    match result {
        Ok(pair) => Ok(Some(pair)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to read auth: {e}")),
    }
}

pub fn delete_auth(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM auth WHERE provider = 'github'", [])
        .map_err(|e| format!("Failed to delete auth: {e}"))?;
    Ok(())
}

// ── Repos ───────────────────────────────────────────────────────────────

pub fn insert_repo(conn: &Connection, repo: &RepoConfig) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO repos (github_id, owner, name, setup_script, run_script, base_branch, branch_prefix, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            repo.github_id,
            repo.owner,
            repo.name,
            repo.setup_script,
            repo.run_script,
            repo.base_branch,
            repo.branch_prefix,
            chrono::Utc::now().to_rfc3339(),
        ],
    )
    .map_err(|e| format!("Failed to insert repo: {e}"))?;

    Ok(conn.last_insert_rowid())
}

/// Get all worktree paths for a repo's sessions.
pub fn get_worktree_paths_for_repo(conn: &Connection, repo_id: i64) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT worktree_path FROM sessions WHERE repo_id = ?1 AND worktree_path IS NOT NULL")
        .map_err(|e| format!("Failed to query worktree paths: {e}"))?;

    let rows = stmt
        .query_map(params![repo_id], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Failed to query worktree paths: {e}"))?;

    let mut paths = Vec::new();
    for row in rows {
        paths.push(row.map_err(|e| format!("Failed to read worktree path: {e}"))?);
    }
    Ok(paths)
}

/// Count sessions for a repo.
pub fn count_sessions_for_repo(conn: &Connection, repo_id: i64) -> Result<i64, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM sessions WHERE repo_id = ?1",
        params![repo_id],
        |row| row.get(0),
    )
    .map_err(|e| format!("Failed to count sessions: {e}"))
}

/// Count session logs for a repo.
pub fn count_session_logs_for_repo(conn: &Connection, repo_id: i64) -> Result<i64, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM session_logs WHERE session_id IN (SELECT id FROM sessions WHERE repo_id = ?1)",
        params![repo_id],
        |row| row.get(0),
    )
    .map_err(|e| format!("Failed to count session logs: {e}"))
}

/// Delete all data associated with a repo (sessions, logs, settings, then the repo itself).
pub fn delete_repo_cascade(conn: &Connection, repo_id: i64) -> Result<(), String> {
    // Delete session logs first (FK to sessions)
    conn.execute(
        "DELETE FROM session_logs WHERE session_id IN (SELECT id FROM sessions WHERE repo_id = ?1)",
        params![repo_id],
    )
    .map_err(|e| format!("Failed to delete session logs: {e}"))?;

    // Delete sessions
    conn.execute("DELETE FROM sessions WHERE repo_id = ?1", params![repo_id])
        .map_err(|e| format!("Failed to delete sessions: {e}"))?;

    // Delete repo-specific settings
    conn.execute(
        "DELETE FROM settings WHERE key = ?1",
        params![format!("repo_{repo_id}_path")],
    )
    .map_err(|e| format!("Failed to delete repo path setting: {e}"))?;

    // Clear selected_repo_id if it points to this repo
    conn.execute(
        "DELETE FROM settings WHERE key = 'selected_repo_id' AND value = ?1",
        params![repo_id.to_string()],
    )
    .map_err(|e| format!("Failed to clear selected repo: {e}"))?;

    // Delete the repo itself
    conn.execute("DELETE FROM repos WHERE id = ?1", params![repo_id])
        .map_err(|e| format!("Failed to delete repo: {e}"))?;

    Ok(())
}

pub fn get_all_repos(conn: &Connection) -> Result<Vec<RepoConfig>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, github_id, owner, name, setup_script, run_script, base_branch, branch_prefix FROM repos",
        )
        .map_err(|e| format!("Failed to query repos: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            let owner: String = row.get(2)?;
            let name: String = row.get(3)?;
            Ok(RepoConfig {
                id: row.get(0)?,
                github_id: row.get(1)?,
                owner: owner.clone(),
                name: name.clone(),
                full_name: format!("{owner}/{name}"),
                setup_script: row.get(4)?,
                run_script: row.get(5)?,
                base_branch: row.get(6)?,
                branch_prefix: row.get(7)?,
            })
        })
        .map_err(|e| format!("Failed to query repos: {e}"))?;

    let mut repos = Vec::new();
    for row in rows {
        repos.push(row.map_err(|e| format!("Failed to read repo row: {e}"))?);
    }
    Ok(repos)
}

pub fn get_repo_by_id(conn: &Connection, repo_id: i64) -> Result<Option<RepoConfig>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, github_id, owner, name, setup_script, run_script, base_branch, branch_prefix FROM repos WHERE id = ?1",
        )
        .map_err(|e| format!("Failed to query repo: {e}"))?;

    let result = stmt.query_row(params![repo_id], |row| {
        let owner: String = row.get(2)?;
        let name: String = row.get(3)?;
        Ok(RepoConfig {
            id: row.get(0)?,
            github_id: row.get(1)?,
            owner: owner.clone(),
            name: name.clone(),
            full_name: format!("{owner}/{name}"),
            setup_script: row.get(4)?,
            run_script: row.get(5)?,
            base_branch: row.get(6)?,
            branch_prefix: row.get(7)?,
        })
    });

    match result {
        Ok(repo) => Ok(Some(repo)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to read repo: {e}")),
    }
}

pub fn update_repo(conn: &Connection, repo: &RepoConfig) -> Result<(), String> {
    conn.execute(
        "UPDATE repos SET setup_script = ?1, run_script = ?2, base_branch = ?3, branch_prefix = ?4 WHERE id = ?5",
        params![
            repo.setup_script,
            repo.run_script,
            repo.base_branch,
            repo.branch_prefix,
            repo.id,
        ],
    )
    .map_err(|e| format!("Failed to update repo: {e}"))?;
    Ok(())
}

// ── Sessions ────────────────────────────────────────────────────────────

pub fn insert_session(conn: &Connection, session: &Session) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO sessions (repo_id, issue_number, stage, worktree_path, session_id, status, error_message, started_at, completed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            session.repo_id,
            session.issue_number,
            session.stage,
            session.worktree_path,
            session.session_id,
            session.status,
            session.error_message,
            session.started_at,
            session.completed_at,
        ],
    )
    .map_err(|e| format!("Failed to insert session: {e}"))?;

    Ok(conn.last_insert_rowid())
}

/// Mark any sessions left in active states as failed (app was quit mid-session).
pub fn fail_orphaned_sessions(conn: &Connection) -> Result<u64, String> {
    let completed_at = chrono::Utc::now().to_rfc3339();
    let count = conn
        .execute(
            "UPDATE sessions SET status = 'failed', error_message = 'App quit while session was running', completed_at = ?1
             WHERE status IN ('running', 'initializing', 'setup')",
            params![completed_at],
        )
        .map_err(|e| format!("Failed to clean up orphaned sessions: {e}"))?;
    Ok(count as u64)
}

pub fn update_session_status(
    conn: &Connection,
    session_db_id: i64,
    status: &str,
    error_message: Option<&str>,
) -> Result<(), String> {
    let completed_at = if status == "completed" || status == "failed" {
        Some(chrono::Utc::now().to_rfc3339())
    } else {
        None
    };

    conn.execute(
        "UPDATE sessions SET status = ?1, error_message = ?2, completed_at = ?3 WHERE id = ?4",
        params![status, error_message, completed_at, session_db_id],
    )
    .map_err(|e| format!("Failed to update session: {e}"))?;
    Ok(())
}

/// Store the Claude CLI session ID so we can resume the conversation later.
pub fn update_session_cli_id(
    conn: &Connection,
    session_db_id: i64,
    cli_session_id: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET session_id = ?1 WHERE id = ?2",
        params![cli_session_id, session_db_id],
    )
    .map_err(|e| format!("Failed to update session CLI ID: {e}"))?;
    Ok(())
}

fn row_to_session(row: &rusqlite::Row) -> rusqlite::Result<Session> {
    Ok(Session {
        id: format!("{}", row.get::<_, i64>(0)?),
        repo_id: row.get(1)?,
        issue_number: row.get(2)?,
        stage: row.get(3)?,
        worktree_path: row.get(4)?,
        session_id: row.get(5)?,
        status: row.get(6)?,
        error_message: row.get(7)?,
        started_at: row.get(8)?,
        completed_at: row.get(9)?,
        hidden: row.get::<_, i64>(10).unwrap_or(0) != 0,
        cost_usd: row.get(11).unwrap_or(None),
    })
}

const SESSION_COLS: &str = "id, repo_id, issue_number, stage, worktree_path, session_id, status, error_message, started_at, completed_at, hidden, cost_usd";

pub fn get_active_session(
    conn: &Connection,
    repo_id: i64,
    issue_number: i64,
) -> Result<Option<Session>, String> {
    let sql = format!(
        "SELECT {SESSION_COLS} FROM sessions WHERE repo_id = ?1 AND issue_number = ?2 AND status IN ('running', 'initializing', 'setup') ORDER BY started_at DESC LIMIT 1"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("Failed to query session: {e}"))?;

    let result = stmt.query_row(params![repo_id, issue_number], row_to_session);

    match result {
        Ok(s) => Ok(Some(s)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to read session: {e}")),
    }
}

pub fn get_latest_session(
    conn: &Connection,
    repo_id: i64,
    issue_number: i64,
) -> Result<Option<Session>, String> {
    let sql = format!(
        "SELECT {SESSION_COLS} FROM sessions WHERE repo_id = ?1 AND issue_number = ?2 ORDER BY started_at DESC LIMIT 1"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("Failed to query session: {e}"))?;

    let result = stmt.query_row(params![repo_id, issue_number], row_to_session);

    match result {
        Ok(s) => Ok(Some(s)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to read session: {e}")),
    }
}

pub fn get_session_by_id(conn: &Connection, session_id: i64) -> Result<Option<Session>, String> {
    let sql = format!("SELECT {SESSION_COLS} FROM sessions WHERE id = ?1");
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("Failed to query session: {e}"))?;

    let result = stmt.query_row(params![session_id], row_to_session);

    match result {
        Ok(s) => Ok(Some(s)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to read session: {e}")),
    }
}

pub fn get_all_sessions(conn: &Connection) -> Result<Vec<Session>, String> {
    let sql = format!("SELECT {SESSION_COLS} FROM sessions WHERE hidden = 0 ORDER BY started_at DESC");
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("Failed to query sessions: {e}"))?;

    let rows = stmt
        .query_map([], row_to_session)
        .map_err(|e| format!("Failed to query sessions: {e}"))?;

    let mut sessions = Vec::new();
    for row in rows {
        sessions.push(row.map_err(|e| format!("Failed to read session row: {e}"))?);
    }
    Ok(sessions)
}

pub fn get_hidden_sessions(conn: &Connection, repo_id: i64, issue_number: i64) -> Result<Vec<Session>, String> {
    let sql = format!(
        "SELECT {SESSION_COLS} FROM sessions WHERE repo_id = ?1 AND issue_number = ?2 AND hidden = 1 ORDER BY started_at DESC"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("Failed to query hidden sessions: {e}"))?;

    let rows = stmt
        .query_map(params![repo_id, issue_number], row_to_session)
        .map_err(|e| format!("Failed to query hidden sessions: {e}"))?;

    let mut sessions = Vec::new();
    for row in rows {
        sessions.push(row.map_err(|e| format!("Failed to read session row: {e}"))?);
    }
    Ok(sessions)
}

pub fn hide_session(conn: &Connection, session_db_id: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET hidden = 1 WHERE id = ?1",
        params![session_db_id],
    )
    .map_err(|e| format!("Failed to hide session: {e}"))?;
    Ok(())
}

pub fn unhide_session(conn: &Connection, session_db_id: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET hidden = 0 WHERE id = ?1",
        params![session_db_id],
    )
    .map_err(|e| format!("Failed to unhide session: {e}"))?;
    Ok(())
}

// ── Session Logs ────────────────────────────────────────────────────────

pub fn insert_session_log(
    conn: &Connection,
    session_db_id: i64,
    event_type: &str,
    content: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO session_logs (session_id, timestamp, event_type, content) VALUES (?1, ?2, ?3, ?4)",
        params![
            session_db_id,
            chrono::Utc::now().to_rfc3339(),
            event_type,
            content,
        ],
    )
    .map_err(|e| format!("Failed to insert session log: {e}"))?;
    Ok(())
}

pub fn get_session_logs(
    conn: &Connection,
    session_db_id: i64,
) -> Result<Vec<SessionLogEntry>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, session_id, timestamp, event_type, content FROM session_logs WHERE session_id = ?1 ORDER BY timestamp ASC",
        )
        .map_err(|e| format!("Failed to query session logs: {e}"))?;

    let rows = stmt
        .query_map(params![session_db_id], |row| {
            Ok(SessionLogEntry {
                id: format!("{}", row.get::<_, i64>(0)?),
                session_id: format!("{}", row.get::<_, i64>(1)?),
                timestamp: row.get(2)?,
                event_type: row.get(3)?,
                content: row.get(4)?,
            })
        })
        .map_err(|e| format!("Failed to query session logs: {e}"))?;

    let mut logs = Vec::new();
    for row in rows {
        logs.push(row.map_err(|e| format!("Failed to read log row: {e}"))?);
    }
    Ok(logs)
}

// ── Settings ────────────────────────────────────────────────────────────

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    let mut stmt = conn
        .prepare("SELECT value FROM settings WHERE key = ?1")
        .map_err(|e| format!("Failed to query setting: {e}"))?;

    let result = stmt.query_row(params![key], |row| row.get::<_, String>(0));

    match result {
        Ok(val) => Ok(Some(val)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to read setting: {e}")),
    }
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )
    .map_err(|e| format!("Failed to set setting: {e}"))?;
    Ok(())
}

pub fn get_app_settings(conn: &Connection) -> Result<AppSettings, String> {
    let defaults = AppSettings::default();

    let sleep = get_setting(conn, "sleep_prevention")?
        .map(|v| v == "true")
        .unwrap_or(defaults.sleep_prevention);
    let notif = get_setting(conn, "notifications_enabled")?
        .map(|v| v == "true")
        .unwrap_or(defaults.notifications_enabled);
    let poll = get_setting(conn, "poll_interval_seconds")?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(defaults.poll_interval_seconds);
    let bypass = get_setting(conn, "bypass_permissions")?
        .map(|v| v == "true")
        .unwrap_or(defaults.bypass_permissions);

    Ok(AppSettings {
        sleep_prevention: sleep,
        notifications_enabled: notif,
        poll_interval_seconds: poll,
        bypass_permissions: bypass,
    })
}

pub fn save_app_settings(conn: &Connection, settings: &AppSettings) -> Result<(), String> {
    set_setting(conn, "sleep_prevention", &settings.sleep_prevention.to_string())?;
    set_setting(conn, "notifications_enabled", &settings.notifications_enabled.to_string())?;
    set_setting(conn, "poll_interval_seconds", &settings.poll_interval_seconds.to_string())?;
    set_setting(conn, "bypass_permissions", &settings.bypass_permissions.to_string())?;
    Ok(())
}

// ── Agent Prompts ───────────────────────────────────────────────────────

pub fn get_all_prompts(conn: &Connection) -> Result<Vec<AgentPrompt>, String> {
    let mut stmt = conn
        .prepare("SELECT stage, prompt_text, is_default, model, effort FROM agent_prompts")
        .map_err(|e| format!("Failed to query prompts: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(AgentPrompt {
                stage: row.get(0)?,
                prompt_text: row.get(1)?,
                is_default: row.get::<_, i32>(2)? != 0,
                model: row.get(3)?,
                effort: row.get(4)?,
            })
        })
        .map_err(|e| format!("Failed to query prompts: {e}"))?;

    let mut prompts = Vec::new();
    for row in rows {
        prompts.push(row.map_err(|e| format!("Failed to read prompt row: {e}"))?);
    }
    Ok(prompts)
}

pub fn get_prompt(conn: &Connection, stage: &str) -> Result<Option<AgentPrompt>, String> {
    let mut stmt = conn
        .prepare("SELECT stage, prompt_text, is_default, model, effort FROM agent_prompts WHERE stage = ?1")
        .map_err(|e| format!("Failed to query prompt: {e}"))?;

    let result = stmt.query_row(params![stage], |row| {
        Ok(AgentPrompt {
            stage: row.get(0)?,
            prompt_text: row.get(1)?,
            is_default: row.get::<_, i32>(2)? != 0,
            model: row.get(3)?,
            effort: row.get(4)?,
        })
    });

    match result {
        Ok(p) => Ok(Some(p)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to read prompt: {e}")),
    }
}

pub fn update_prompt(conn: &Connection, stage: &str, prompt_text: &str, model: &str, effort: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE agent_prompts SET prompt_text = ?1, is_default = 0, model = ?3, effort = ?4 WHERE stage = ?2",
        params![prompt_text, stage, model, effort],
    )
    .map_err(|e| format!("Failed to update prompt: {e}"))?;
    Ok(())
}
