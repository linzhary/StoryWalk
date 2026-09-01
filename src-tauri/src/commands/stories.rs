use crate::db;
use crate::error::AppError;
use crate::materials;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Story {
    pub id: String,
    pub title: String,
    pub description: String,
    /// 故事模式：card（写卡，正文沉淀为剧情卡片）/ chat（纯聊，正文直接回复在聊天框）
    pub mode: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BreadcrumbStory {
    pub id: String,
    pub title: String,
}

#[tauri::command]
pub fn get_stories() -> Result<Vec<Story>, AppError> {
    db::with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, title, description, mode, created_at, updated_at FROM stories ORDER BY updated_at DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Story {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                mode: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        rows.collect()
    })
    .map_err(AppError::Database)
}

#[tauri::command]
pub fn get_story(story_id: String) -> Result<Story, AppError> {
    db::with_db(|conn| {
        conn.query_row(
            "SELECT id, title, description, mode, created_at, updated_at FROM stories WHERE id = ?1",
            [&story_id],
            |row| {
                Ok(Story {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    mode: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            },
        )
    })
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound("故事不存在".into()),
        other => AppError::Database(other),
    })
}

#[tauri::command]
pub fn create_story(
    title: String,
    description: String,
    style: Option<String>,
    mode: Option<String>,
) -> Result<Story, AppError> {
    let id = db::gen_id("story");
    let story_id = id.clone();
    // 故事模式：card（写卡，默认）/ chat（纯聊）
    let mode = mode.unwrap_or_else(|| "card".into());
    db::with_db(|conn| {
        conn.execute(
            "INSERT INTO stories (id, title, description, mode) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, title, description, mode],
        )?;
        // Auto-create the single writing session (settings/extraction sessions are created lazily)
        let writing_session_id = db::gen_id("session");
        conn.execute(
            "INSERT INTO story_sessions (id, story_id, title, mode, model) VALUES (?1, ?2, ?3, 'creation', 'deepseek-v4-flash')",
            rusqlite::params![writing_session_id, story_id, "写作会话"],
        )?;
        Ok(())
    })
    .map_err(AppError::Database)?;
    // Initialize story directory for MD files
    materials::init_story_dir(&story_id).ok();
    // Seed guidelines.md with the chosen style (modern / ancient per selection)
    let seed = materials::build_guidelines_seed(style.as_deref());
    materials::update_story_md(&story_id, "guidelines", &seed).ok();
    get_story(story_id)
}

#[tauri::command]
pub fn update_story(story_id: String, title: Option<String>, description: Option<String>) -> Result<Story, AppError> {
    db::with_db(|conn| {
        if let Some(ref t) = title {
            conn.execute("UPDATE stories SET title = ?1, updated_at = datetime('now') WHERE id = ?2", rusqlite::params![t, story_id])?;
        }
        if let Some(ref d) = description {
            conn.execute("UPDATE stories SET description = ?1, updated_at = datetime('now') WHERE id = ?2", rusqlite::params![d, story_id])?;
        }
        Ok(())
    })
    .map_err(AppError::Database)?;
    get_story(story_id)
}

#[tauri::command]
pub fn delete_story(story_id: String) -> Result<(), AppError> {
    db::with_db(|conn| {
        conn.execute("DELETE FROM story_cards WHERE story_id = ?1", [&story_id])?;
        conn.execute(
            "DELETE FROM story_messages WHERE session_id IN (SELECT id FROM story_sessions WHERE story_id = ?1)",
            [&story_id],
        )?;
        conn.execute("DELETE FROM story_sessions WHERE story_id = ?1", [&story_id])?;
        conn.execute("DELETE FROM stories WHERE id = ?1", [&story_id])?;
        Ok(())
    })
    .map_err(AppError::Database)?;
    // Delete story directory (materials: reference.md / guidelines.md)
    if let Err(e) = materials::delete_story_dir(&story_id) {
        eprintln!("[stories] delete story dir failed: story={} err={}", story_id, e);
    }
    Ok(())
}

#[tauri::command]
pub fn get_breadcrumb_story(story_id: String) -> Result<BreadcrumbStory, AppError> {
    db::with_db(|conn| {
        conn.query_row(
            "SELECT id, title FROM stories WHERE id = ?1",
            [&story_id],
            |row| {
                Ok(BreadcrumbStory {
                    id: row.get(0)?,
                    title: row.get(1)?,
                })
            },
        )
    })
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound("故事不存在".into()),
        other => AppError::Database(other),
    })
}
