use crate::db;
use crate::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoryCard {
    pub id: String,
    #[serde(rename = "storyId")]
    pub story_id: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub content: String,
    #[serde(rename = "roundNumber")]
    pub round_number: i32,
    pub created_at: String,
}

#[tauri::command]
pub fn get_story_cards(story_id: String) -> Result<Vec<StoryCard>, AppError> {
    db::with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, story_id, session_id, content, round_number, created_at \
             FROM story_cards WHERE story_id = ?1 ORDER BY round_number ASC"
        )?;
        let rows = stmt.query_map(rusqlite::params![story_id], |row| {
            Ok(StoryCard {
                id: row.get(0)?,
                story_id: row.get(1)?,
                session_id: row.get(2)?,
                content: row.get(3)?,
                round_number: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect()
    })
    .map_err(AppError::Database)
}

/// Look up a single story card by id. Used by the creation-mode `read_story_card` AI tool.
pub fn get_story_card(card_id: String) -> Result<StoryCard, AppError> {
    db::with_db(|conn| {
        conn.query_row(
            "SELECT id, story_id, session_id, content, round_number, created_at FROM story_cards WHERE id = ?1",
            [&card_id],
            |row| {
                Ok(StoryCard {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    session_id: row.get(2)?,
                    content: row.get(3)?,
                    round_number: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
    })
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound("卡片不存在".into()),
        other => AppError::Database(other),
    })
}

/// Look up the story card of a specific round (rounds are unique per story).
/// Used by the creation-mode `read_story_card` AI tool when the user references
/// a card by `[第N轮]` / `[第N轮:S-E]` tag (which carries no card id).
pub fn get_story_card_by_round(story_id: String, round: i32) -> Result<StoryCard, AppError> {
    db::with_db(|conn| {
        conn.query_row(
            "SELECT id, story_id, session_id, content, round_number, created_at FROM story_cards WHERE story_id = ?1 AND round_number = ?2 ORDER BY created_at DESC LIMIT 1",
            rusqlite::params![story_id, round],
            |row| {
                Ok(StoryCard {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    session_id: row.get(2)?,
                    content: row.get(3)?,
                    round_number: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
    })
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound("该轮次卡片不存在".into()),
        other => AppError::Database(other),
    })
}

/// Shared card creation logic, reused by the `save_story_card` command and
/// the creation-mode `save_story_card` AI tool.
pub fn create_story_card(story_id: String, session_id: String, content: String) -> Result<StoryCard, AppError> {
    let id = db::gen_id("card");
    let cid = id.clone();
    let sid = story_id.clone();

    db::with_db(move |conn| {
        let max_round: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(round_number), 0) + 1 FROM story_cards WHERE story_id = ?1",
                rusqlite::params![sid],
                |row| row.get(0),
            )
            .unwrap_or(1);
        conn.execute(
            "INSERT INTO story_cards (id, story_id, session_id, content, round_number) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, story_id, session_id, content, max_round],
        )?;
        Ok(())
    })
    .map_err(AppError::Database)?;

    db::with_db(|conn| {
        conn.query_row(
            "SELECT id, story_id, session_id, content, round_number, created_at FROM story_cards WHERE id = ?1",
            [&cid],
            |row| {
                Ok(StoryCard {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    session_id: row.get(2)?,
                    content: row.get(3)?,
                    round_number: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
    })
    .map_err(AppError::Database)
}

#[tauri::command]
pub fn save_story_card(story_id: String, session_id: String, content: String) -> Result<StoryCard, AppError> {
    create_story_card(story_id, session_id, content)
}

#[tauri::command]
pub fn update_story_card(card_id: String, content: String) -> Result<StoryCard, AppError> {
    let cid = card_id.clone();
    db::with_db(move |conn| {
        conn.execute(
            "UPDATE story_cards SET content = ?1 WHERE id = ?2",
            rusqlite::params![content, cid],
        )?;
        Ok(())
    })
    .map_err(AppError::Database)?;

    db::with_db(|conn| {
        conn.query_row(
            "SELECT id, story_id, session_id, content, round_number, created_at FROM story_cards WHERE id = ?1",
            [&card_id],
            |row| {
                Ok(StoryCard {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    session_id: row.get(2)?,
                    content: row.get(3)?,
                    round_number: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
    })
    .map_err(AppError::Database)
}

#[tauri::command]
pub fn delete_story_card(card_id: String) -> Result<(), AppError> {
    db::with_db(|conn| {
        conn.execute("DELETE FROM story_cards WHERE id = ?1", [&card_id])?;
        Ok(())
    })
    .map_err(AppError::Database)
}
