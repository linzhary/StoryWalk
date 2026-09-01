use crate::db;
use crate::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct StorySession {
    pub id: String,
    #[serde(rename = "storyId")]
    pub story_id: String,
    pub title: String,
    pub mode: String,
    pub model: String,
    pub summary: String,
    pub created_at: String,
}

#[tauri::command]
pub fn get_sessions(story_id: String) -> Result<Vec<StorySession>, AppError> {
    db::with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, story_id, title, mode, model, summary, created_at FROM story_sessions WHERE story_id = ?1 ORDER BY created_at DESC"
        )?;
        let rows = stmt.query_map([&story_id], |row| {
            Ok(StorySession {
                id: row.get(0)?,
                story_id: row.get(1)?,
                title: row.get(2)?,
                mode: row.get(3)?,
                model: row.get(4)?,
                summary: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        rows.collect()
    })
    .map_err(AppError::Database)
}

#[tauri::command]
pub fn create_session(
    story_id: String,
    title: Option<String>,
    mode: Option<String>,
    model: Option<String>,
) -> Result<StorySession, AppError> {
    let id = db::gen_id("session");
    let title = title.unwrap_or_else(|| "新会话".into());
    let mode = mode.unwrap_or_else(|| "creation".into());
    let model = model.unwrap_or_else(|| "deepseek-v4-flash".into());

    let session_id = id.clone();
    db::with_db(move |conn| {
        conn.execute(
            "INSERT INTO story_sessions (id, story_id, title, mode, model) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, story_id, title, mode, model],
        )
    })
    .map_err(AppError::Database)?;

    db::with_db(|conn| {
        conn.query_row(
            "SELECT id, story_id, title, mode, model, summary, created_at FROM story_sessions WHERE id = ?1",
            [&session_id],
            |row| {
                Ok(StorySession {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    title: row.get(2)?,
                    mode: row.get(3)?,
                    model: row.get(4)?,
                    summary: row.get(5)?,
                    created_at: row.get(6)?,
                })
            },
        )
    })
    .map_err(AppError::Database)
}

#[tauri::command]
pub fn update_session(
    session_id: String,
    title: Option<String>,
    model: Option<String>,
) -> Result<StorySession, AppError> {
    let sid = session_id.clone();
    db::with_db(move |conn| {
        if let Some(ref t) = title {
            conn.execute("UPDATE story_sessions SET title = ?1 WHERE id = ?2", rusqlite::params![t, sid])?;
        }
        if let Some(ref m) = model {
            conn.execute("UPDATE story_sessions SET model = ?1 WHERE id = ?2", rusqlite::params![m, sid])?;
        }
        Ok(())
    })
    .map_err(AppError::Database)?;

    db::with_db(|conn| {
        conn.query_row(
            "SELECT id, story_id, title, mode, model, summary, created_at FROM story_sessions WHERE id = ?1",
            [&session_id],
            |row| {
                Ok(StorySession {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    title: row.get(2)?,
                    mode: row.get(3)?,
                    model: row.get(4)?,
                    summary: row.get(5)?,
                    created_at: row.get(6)?,
                })
            },
        )
    })
    .map_err(AppError::Database)
}

#[tauri::command]
pub fn delete_session(session_id: String) -> Result<(), AppError> {
    db::with_db(|conn| {
        conn.execute("DELETE FROM story_messages WHERE session_id = ?1", [&session_id])?;
        conn.execute("DELETE FROM story_sessions WHERE id = ?1", [&session_id])?;
        Ok(())
    })
    .map_err(AppError::Database)
}
