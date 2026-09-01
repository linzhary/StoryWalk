//! 上下文压缩（参考 BitFun ContextCompressor 机制，适配 DB 化架构）。
//!
//! 与旧版"全量总结全部标记"不同，本模块在自动压缩时：
//! - 按原子单元（assistant + 其后 tool 消息）切分，压缩边界不切断工具配对；
//! - 保留最近约 `RECENT_TAIL_TOKENS` token 的尾部消息逐字回传；
//! - 在待总结部分按预算逐字保留最近的用户消息；
//! - 其余消息总结为摘要（模型失败时回退本地截断），并标记 `summarized = 1`。
//!
//! 摘要写入 `story_sessions.summary`，请求时由各阶段按现有机制注入。

use crate::db;
use crate::deepseek;
use crate::error::AppError;
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// 保留的最近尾部 token 预算（对应 BitFun `DEFAULT_RECENT_CONTEXT_TOKENS`）。
const RECENT_TAIL_TOKENS: usize = 10_000;
/// 待总结部分中逐字保留用户消息的预算（context_window / 10，按 128K 上下文估算）。
const RETAINED_USER_TOKEN_BUDGET: usize = 12_800;
/// 保留用户消息的硬上限（对应 BitFun `MAX_RETAINED_USER_TOKENS`）。
const MAX_RETAINED_USER_TOKENS: usize = 20_000;
/// 模型总结失败时的本地回退摘要：最多取最近 5 条用户消息，每条截断 200 字符。
const FALLBACK_USER_MESSAGES: usize = 5;
const FALLBACK_USER_CHARS: usize = 200;
/// 摘要边界说明（对应 BitFun `render_boundary_marker_text`）。
const SUMMARY_BOUNDARY_TEXT: &str =
    "【压缩说明】部分较早消息已逐字保留，其余早期对话已总结为以下摘要。若需要被总结内容的细节，可重新读取素材文件或直接提问。";

/// 简单的 token 估算：混合文本按 2 字符 1 token 保守近似（中文约 1 字 1 token）。
pub fn estimate_text_tokens(s: &str) -> usize {
    (s.chars().count() / 2).max(1)
}

struct MsgRow {
    id: String,
    role: String,
    content: String,
    reasoning: String,
    tool_calls: String,
    tokens: usize,
}

struct AtomicUnit {
    start: usize,
    tokens: usize,
}

/// 原子单元切分：assistant 及其后连续的 tool 消息为一个单元，压缩边界不切断工具配对。
fn atomic_units(rows: &[MsgRow]) -> Vec<AtomicUnit> {
    let mut units = Vec::new();
    let mut i = 0;
    while i < rows.len() {
        let start = i;
        i += 1;
        if rows[start].role == "assistant" {
            while i < rows.len() && rows[i].role == "tool" {
                i += 1;
            }
        }
        let tokens: usize = rows[start..i].iter().map(|r| r.tokens).sum();
        units.push(AtomicUnit { start, tokens });
    }
    units
}

pub struct CompressionPlan {
    /// 待总结并标记 summarized 的消息 id
    pub summarize_ids: Vec<String>,
    /// 保留（不标记）的消息 id：最近尾部 + 待总结部分中逐字保留的用户消息
    pub keep_ids: Vec<String>,
    /// 送给模型的总结输入（user/assistant 文本，跳过 tool）
    pub summary_input: String,
    /// 模型总结失败时的本地回退输入（待总结部分最近的用户消息，截断拼接）
    pub fallback_input: String,
}

/// 规划一次压缩：返回 None 表示无事可做（消息为空或全部应保留）。
pub fn plan_compression(session_id: &str) -> Result<Option<CompressionPlan>, AppError> {
    let mut rows: Vec<MsgRow> = db::with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, role, content, reasoning, tool_calls FROM story_messages \
             WHERE session_id = ?1 AND summarized = 0 \
             ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = stmt.query_map([session_id], |row| {
            Ok(MsgRow {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                reasoning: row.get(3)?,
                tool_calls: row.get(4)?,
                tokens: 0,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    })
    .map_err(AppError::Database)?;

    if rows.is_empty() {
        return Ok(None);
    }
    for r in &mut rows {
        r.tokens = estimate_text_tokens(&r.content)
            .saturating_add(estimate_text_tokens(&r.reasoning))
            .saturating_add(estimate_text_tokens(&r.tool_calls));
    }

    let units = atomic_units(&rows);

    // 尾部保留：从尾部往回累计单元 token，超出预算前的位置作为 cutoff。
    // summary 部分至少保留第一个单元（与 BitFun minimum_cutoff 一致）。
    let minimum_cutoff = if units.len() > 1 { units[1].start } else { rows.len() };
    let mut cutoff = rows.len();
    let mut accumulated = 0usize;
    for unit in units.iter().rev() {
        if unit.start < minimum_cutoff {
            break;
        }
        let next = accumulated.saturating_add(unit.tokens);
        if next > RECENT_TAIL_TOKENS {
            break;
        }
        cutoff = unit.start;
        accumulated = next;
    }

    let summary_rows = &rows[..cutoff];
    if summary_rows.is_empty() {
        return Ok(None);
    }
    let tail_rows = &rows[cutoff..];

    // 保留集合：尾部消息 + 待总结部分中按预算逐字保留的最近用户消息
    let mut keep_ids: Vec<String> = tail_rows.iter().map(|r| r.id.clone()).collect();
    let budget = RETAINED_USER_TOKEN_BUDGET.min(MAX_RETAINED_USER_TOKENS);
    let mut retained_tokens = 0usize;
    for r in summary_rows.iter().rev() {
        if r.role != "user" {
            continue;
        }
        let next = retained_tokens.saturating_add(r.tokens);
        if next > budget {
            break;
        }
        retained_tokens = next;
        keep_ids.push(r.id.clone());
    }

    let keep_set: HashSet<String> = keep_ids.iter().cloned().collect();
    let summarize_ids: Vec<String> = summary_rows
        .iter()
        .map(|r| r.id.clone())
        .filter(|id| !keep_set.contains(id))
        .collect();
    if summarize_ids.is_empty() {
        return Ok(None);
    }

    // 总结输入：待总结的 user/assistant 文本（跳过 tool），与旧版 do_summarize 格式一致
    let summary_input = summary_rows
        .iter()
        .filter(|r| r.role != "tool" && !keep_set.contains(&r.id))
        .map(|r| format!("{}: {}", r.role, r.content))
        .collect::<Vec<_>>()
        .join("\n\n");
    if summary_input.trim().is_empty() {
        return Ok(None);
    }

    // 回退输入：待总结部分最近的用户消息（截断拼接）
    let fallback_input = summary_rows
        .iter()
        .rev()
        .filter(|r| r.role == "user" && !keep_set.contains(&r.id))
        .take(FALLBACK_USER_MESSAGES)
        .map(|r| r.content.chars().take(FALLBACK_USER_CHARS).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n\n");

    println!(
        "[compression] plan: session={} total_msgs={} summarize={} keep={} (tail_tokens={})",
        session_id,
        rows.len(),
        summarize_ids.len(),
        keep_ids.len(),
        accumulated,
    );

    Ok(Some(CompressionPlan {
        summarize_ids,
        keep_ids,
        summary_input,
        fallback_input,
    }))
}

/// 执行一次压缩：模型总结待总结部分，成功后标记 summarized 并写入 session.summary。
/// 模型总结失败时回退本地截断摘要（仍然标记，避免反复触发）。
pub async fn execute_compression(
    client: &reqwest::Client,
    api_key: &str,
    session_id: &str,
) -> Result<String, AppError> {
    let Some(plan) = plan_compression(session_id)? else {
        return Ok(String::new());
    };

    let summary = match generate_model_summary(client, api_key, &plan.summary_input).await {
        Ok(s) if !s.trim().is_empty() => {
            format!("{}\n\n{}", SUMMARY_BOUNDARY_TEXT, s.trim())
        }
        _ => {
            // 本地回退：结构化截断
            let fallback = if plan.fallback_input.trim().is_empty() {
                plan.summary_input.chars().take(500).collect::<String>()
            } else {
                plan.fallback_input.clone()
            };
            println!(
                "[compression] model summary failed, using local fallback: session={}",
                session_id
            );
            format!("{}\n\n{}", SUMMARY_BOUNDARY_TEXT, fallback)
        }
    };

    db::with_db(|conn| {
        for id in &plan.summarize_ids {
            conn.execute("UPDATE story_messages SET summarized = 1 WHERE id = ?1", [id])?;
        }
        conn.execute(
            "UPDATE story_sessions SET summary = ?1 WHERE id = ?2",
            rusqlite::params![summary, session_id],
        )?;
        Ok(())
    })
    .map_err(AppError::Database)?;

    println!(
        "[compression] done: session={} summarized={} summary_chars={}",
        session_id,
        plan.summarize_ids.len(),
        summary.chars().count(),
    );

    Ok(summary)
}

/// 用模型生成摘要（非流式，静默收集）。
async fn generate_model_summary(
    client: &reqwest::Client,
    api_key: &str,
    conversation: &str,
) -> Result<String, String> {
    let messages = vec![
        serde_json::json!({
            "role": "system",
            "content": crate::chat::CREATION_SUMMARY_PROMPT
        }),
        serde_json::json!({
            "role": "user",
            "content": format!("请总结以下对话：\n\n{}", conversation)
        }),
    ];
    let result = deepseek::stream_chat(
        client,
        api_key,
        "deepseek-v4-flash",
        &messages,
        &[],
        |_| {},
        Arc::new(AtomicBool::new(false)),
    )
    .await?;
    Ok(result.content)
}
