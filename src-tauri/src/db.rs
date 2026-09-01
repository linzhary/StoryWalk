use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

static DB: std::sync::OnceLock<Mutex<Connection>> = std::sync::OnceLock::new();

pub fn init_db(app_dir: &PathBuf) -> rusqlite::Result<()> {
    let db_path = app_dir.join("data.db");
    println!("[DB] Opening: {}", db_path.display());
    
    // Check file size before opening
    if let Ok(meta) = std::fs::metadata(&db_path) {
        println!("[DB] File size: {} bytes", meta.len());
    }
    
    // Count messages before init
    if let Ok(conn) = Connection::open(&db_path) {
        let count: Result<i64, _> = conn.query_row("SELECT COUNT(*) FROM story_messages", [], |r| r.get(0));
        if let Ok(c) = count {
            println!("[DB] Messages before init: {}", c);
        }
        drop(conn);
    }
    
    // Clean up stale WAL/SHM files that may conflict after git operations
    let _ = std::fs::remove_file(app_dir.join("data.db-wal"));
    let _ = std::fs::remove_file(app_dir.join("data.db-shm"));
    let conn = Connection::open(&db_path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = DELETE;")?;
    init_schema(&conn)?;
    
    // Count messages after init
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM story_messages", [], |r| r.get(0)).unwrap_or(-1);
    println!("[DB] Messages after init: {}", count);
    
    DB.set(Mutex::new(conn)).map_err(|_| {
        rusqlite::Error::InvalidParameterName("DB already initialized".into())
    })?;
    Ok(())
}

pub fn with_db<F, T>(f: F) -> Result<T, rusqlite::Error>
where
    F: FnOnce(&Connection) -> Result<T, rusqlite::Error>,
{
    let lock = DB.get().expect("DB not initialized");
    let conn = lock.lock().unwrap();
    f(&conn)
}

/// 当前 schema 版本。这是首次创建新库的基线版本:v1 即当前最新结构,
/// 由 `init_schema` 中的 CREATE TABLE 直接创建。
/// 未来结构变更时递增此值,并在 `migrate()` 中追加对应版本段。
const SCHEMA_VERSION: i64 = 2;

fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS stories (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT DEFAULT '',
            mode TEXT DEFAULT 'card',
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS story_sessions (
            id TEXT PRIMARY KEY,
            story_id TEXT NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
            title TEXT DEFAULT '新会话',
            mode TEXT DEFAULT 'creation',
            model TEXT DEFAULT 'deepseek-v4-flash',
            summary TEXT DEFAULT '',
            created_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS story_messages (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL REFERENCES story_sessions(id) ON DELETE CASCADE,
            role TEXT NOT NULL,
            content TEXT NOT NULL DEFAULT '',
            reasoning TEXT DEFAULT '',
            tool_call_id TEXT,
            tool_calls TEXT DEFAULT '',
            summarized INTEGER DEFAULT 0,
            dedup_key TEXT DEFAULT '',
            phase TEXT DEFAULT 'creation',
            created_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS story_cards (
            id TEXT PRIMARY KEY,
            story_id TEXT NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
            session_id TEXT NOT NULL REFERENCES story_sessions(id) ON DELETE CASCADE,
            content TEXT NOT NULL DEFAULT '',
            round_number INTEGER NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        );
    ")?;

    migrate(conn)
}

/// 版本化迁移:每个版本段只在 `PRAGMA user_version` 低于该版本时执行一次,
/// 完成后将 user_version 更新到 SCHEMA_VERSION。新库首次创建时 user_version
/// 为 0,执行 v1 基线段后标记完成;之后每次启动直接跳过,零操作。
///
/// 原则:迁移只负责结构升级,不删除用户数据
/// (story_messages 中的记录,包括工具调用消息,一律保留)。
fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);

    if version < 1 {
        // v1: 首次创建基线。新库由 init_schema 中的 CREATE TABLE 直接建
        // 全量结构,此处无需额外语句。未来结构变更时,在本段之后追加
        // 新的版本段,并递增 SCHEMA_VERSION。
    }

    if version < 2 {
        // v2: stories 表增加 mode 列（写卡 card / 纯聊 chat，默认 card）。
        // 新库由 CREATE TABLE 直接建列，老库在此处补列（带存在性检查）。
        let has_mode: bool = conn
            .prepare("SELECT 1 FROM pragma_table_info('stories') WHERE name = 'mode'")
            .and_then(|mut stmt| stmt.exists([]))
            .unwrap_or(false);
        if !has_mode {
            conn.execute("ALTER TABLE stories ADD COLUMN mode TEXT DEFAULT 'card'", [])?;
        }
    }

    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

/// Generate a unique ID with prefix and timestamp
pub fn gen_id(prefix: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("{}-{}", prefix, ts)
}
