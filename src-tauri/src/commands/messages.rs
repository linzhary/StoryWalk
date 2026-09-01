use crate::db;
use crate::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoryMessage {
    pub id: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub reasoning: Option<String>,
    #[serde(rename = "toolCallId")]
    pub tool_call_id: Option<String>,
    pub summarized: Option<i32>,
    pub phase: Option<String>,
    pub created_at: String,
}

#[tauri::command]
pub fn get_messages(session_id: String) -> Result<Vec<StoryMessage>, AppError> {
    db::with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, content, reasoning, tool_call_id, summarized, phase, created_at \
             FROM story_messages WHERE session_id = ?1 AND summarized = 0 ORDER BY rowid ASC"
        )?;
        let rows = stmt.query_map([&session_id], |row| {
            Ok(StoryMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                reasoning: row.get(4)?,
                tool_call_id: row.get(5)?,
                summarized: row.get(6)?,
                phase: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        rows.collect()
    })
    .map_err(AppError::Database)
}

#[tauri::command]
pub fn get_message_count(session_id: String) -> Result<i64, AppError> {
    db::with_db(|conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM story_messages WHERE session_id = ?1 AND summarized = 0",
            [&session_id],
            |row| row.get(0),
        )
    })
    .map_err(AppError::Database)
}

#[tauri::command]
pub fn get_messages_paginated(
    session_id: String,
    before_id: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<StoryMessage>, AppError> {
    let limit = limit.unwrap_or(50);
    db::with_db(|conn| {
        let mut messages: Vec<StoryMessage> = if let Some(ref bid) = before_id {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, role, content, reasoning, tool_call_id, summarized, phase, created_at \
                 FROM story_messages WHERE session_id = ?1 AND summarized = 0 \
                 AND rowid < (SELECT rowid FROM story_messages WHERE id = ?2) \
                 ORDER BY rowid DESC LIMIT ?3"
            )?;
            let rows = stmt.query_map(rusqlite::params![session_id, bid, limit], |row| {
                Ok(StoryMessage {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    reasoning: row.get(4)?,
                    tool_call_id: row.get(5)?,
                    summarized: row.get(6)?,
                    phase: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, role, content, reasoning, tool_call_id, summarized, phase, created_at \
                 FROM story_messages WHERE session_id = ?1 AND summarized = 0 \
                 ORDER BY rowid DESC LIMIT ?2"
            )?;
            let rows = stmt.query_map(rusqlite::params![session_id, limit], |row| {
                Ok(StoryMessage {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    reasoning: row.get(4)?,
                    tool_call_id: row.get(5)?,
                    summarized: row.get(6)?,
                    phase: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        messages.reverse();
        Ok(messages)
    })
    .map_err(AppError::Database)
}

#[tauri::command]
pub fn save_message(
    session_id: String,
    role: String,
    content: String,
    reasoning: Option<String>,
    tool_call_id: Option<String>,
    phase: Option<String>,
) -> Result<StoryMessage, AppError> {
    let id = db::gen_id("msg");
    let msg_id = id.clone();
    let phase_val = phase.unwrap_or_else(|| "creation".into());
    db::with_db(move |conn| {
        conn.execute(
            "INSERT INTO story_messages (id, session_id, role, content, reasoning, tool_call_id, phase) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![id, session_id, role, content, reasoning, tool_call_id, phase_val],
        )
    })
    .map_err(AppError::Database)?;

    db::with_db(|conn| {
        conn.query_row(
            "SELECT id, session_id, role, content, reasoning, tool_call_id, summarized, phase, created_at FROM story_messages WHERE id = ?1",
            [&msg_id],
            |row| {
                Ok(StoryMessage {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    reasoning: row.get(4)?,
                    tool_call_id: row.get(5)?,
                    summarized: row.get(6)?,
                    phase: row.get(7)?,
                    created_at: row.get(8)?,
                })
            },
        )
    })
    .map_err(AppError::Database)
}

#[tauri::command]
pub fn rollback_messages(session_id: String, message_id: String) -> Result<Vec<StoryMessage>, AppError> {
    db::with_db(|conn| {
        let target_rowid: Option<i64> = conn
            .query_row(
                "SELECT rowid FROM story_messages WHERE id = ?1 AND session_id = ?2",
                rusqlite::params![message_id, session_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(rowid) = target_rowid {
            conn.execute(
                "DELETE FROM story_messages WHERE session_id = ?1 AND rowid >= ?2",
                rusqlite::params![session_id, rowid],
            )?;
        }
        Ok(())
    })
    .map_err(AppError::Database)?;
    get_messages(session_id)
}

#[tauri::command]
pub fn delete_message(message_id: String) -> Result<(), AppError> {
    db::with_db(|conn| {
        conn.execute("DELETE FROM story_messages WHERE id = ?1", [&message_id])?;
        Ok(())
    })
    .map_err(AppError::Database)
}
