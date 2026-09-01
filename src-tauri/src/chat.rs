use crate::commands::story_cards;
use crate::db;
use crate::deepseek::{self, StreamEvent};
use crate::error::AppError;
use crate::materials;
use crate::CancelState;
use async_openai::types::{ChatCompletionTool, ChatCompletionToolType, FunctionObject};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

/// 写作模式 system prompt：任务规则 + 核心剧情驱动规则 + 参考资料（运行时填充），
/// 以 `<互动历史>` 开标签结尾（不闭合），闭合标签在最新用户消息中。
const CREATION_TASK_PROMPT: &str = r#"这是一个由角色性格驱动、用户主导方向的创意写作任务。包含诸多设定、写作指导和格式要求。

【核心剧情驱动规则】剧情完全由人物性格 + 用户输入驱动。你的任务是在角色设定和创作准则的约束下，根据用户的输入发展符合角色定义的剧情。不得预设或强制推进特定剧情走向——剧情方向由用户输入触发，但剧情的内容和发展必须严格符合角色的性格设定，由角色性格自然地推动剧情走向。

【创作流程——先确认大纲，再写作】
1. 用户给出剧情方向或简介后，第一步先输出本段剧情的简要大纲（2-4 条要点：场景、人物状态、情节推进、情绪基调），等待用户确认
2. 用户确认大纲（或明确表示"直接写"）后，再写作正文并通过 save_story_card 保存
3. 若用户明确要求直接写作（如"直接写""不用确认"），可跳过大纲确认
4. 大纲阶段不调用任何工具，仅输出要点列表；不要在大纲中展开正文内容

【输出格式要求】
- 剧情正文只通过 save_story_card 工具写入创作卡片，不要在回复内容中直接输出正文
- 回复内容仅用于简短确认（如"本段剧情已写入卡片"）、简要说明本段剧情要点、或提出下一步建议与澄清问题
- 正文使用连贯的叙事段落描写场景、动作、对话、心理与情节推进，不加章节标题、分节标题、"第一章/第二章"之类的结构标记或编号
- 正文中不要混入大纲、梗概、写作思路、剧情说明或任何与正文无关的文字
- 每次只写用户要求的那一段剧情，一段完整的正文

你不是故事中的任何角色，而是与User合作的创作者。请严格遵循以下指引，以确保最佳体验。

下面是一些参考资料和互动历史，请仔细阅读：
<参考资料>
{REFERENCE}
</参考资料>
<创作准则>
{GUIDELINES}
</创作准则>
{TOOL_AND_THINKING}
<互动历史>
"#;

/// 文风素材（现代/古代两个容器 + 独立规则/约束文件）：
/// 容器文件末尾含 `<补充规则/>`、`<自省约束/>` 占位及 `</写作文风>` 之后的
/// `<示例对白/>` 自闭合占位，由 build_guidelines_seed 替换为通用规则/约束/对白库内容。
pub const CREATION_STYLE_MODERN: &str = include_str!("../../prompts/detailed-modern.md");
pub const CREATION_STYLE_ANCIENT: &str = include_str!("../../prompts/detailed-ancient.md");
pub const CREATION_STYLE_RULES: &str = include_str!("../../prompts/rules.md");
pub const CREATION_STYLE_SELF_CHECK: &str = include_str!("../../prompts/self-check.md");

/// 示例对白库（剧情叙事对白，古风版）：替换 `</写作文风>` 之后的 `<示例对白/>` 占位，
/// 内容为 `<示例对白>…</示例对白>` 标签（5 类叙事向轻量对白，平辈角色沈砚/苏念）。
pub const CREATION_STYLE_ANCIENT_DIALOGUE: &str = include_str!("../../prompts/novel-dialogue-ancient.md");

/// 示例对白库（剧情叙事对白，现代版）：古风示例的现代改写（林晚/陈默），
/// 由 build_guidelines_seed 按用户选择的文风注入 modern 容器。
pub const CREATION_STYLE_MODERN_DIALOGUE: &str = include_str!("../../prompts/novel-dialogue-modern.md");

/// 创建故事时 guidelines.md 初始化的固定创作准则段（# 创作准则 故事专属区域），
/// 由 build_guidelines_seed 拼接在 <写作文风> 之后。
pub const CREATION_INITIAL_GUIDELINES: &str = include_str!("../../prompts/initial-guidelines.md");

/// 写作模式工具说明（写卡模式）：注入 system prompt（<创作准则> 之后、<互动历史> 之前）。
const CREATION_TOOL_DESCRIPTION: &str = r#"【工具说明】
本会话提供四个工具：
1. read_story_cards — 读取本故事已保存的全部剧情卡片（按轮次从早到晚），用于回顾已写剧情、保持故事连续性。仅在需要回顾前文（如不确定已写内容、轮次间隔较久）时调用，不要每轮重复调用；可传 last_n 只取最近 N 张
2. read_story_card — 按轮次读取单张剧情卡片或片段。当用户消息中出现 [第N轮] 引用标记时，传 round=N 读取整卡；出现 [第N轮:起始-结束] 标记时，传 round=N 及 start/end 读取对应片段（聚焦该片段处理）；旧格式 [卡片:第N轮](card-xxx) 时传 card_id 读取该卡片。读取结果会返回卡片 id，供 update_story_card 使用
3. update_story_card — 按卡片 ID 更新卡片内容。用户要求修改已保存的剧情时使用：先调用 read_story_card 读取原内容，修改后通过本工具写回；更新后的正文仍只通过工具写入，不在回复中输出正文
4. save_story_card — 新剧情正文必须通过该工具写入创作卡片：content 参数保存正文本身，不含章节标题、分节标题、大纲、注释或任何说明性文字。回复内容中不得直接输出正文，只需简短确认、说明要点或提问。澄清提问、讨论分析阶段不得调用该工具。调用本工具前，需先向用户输出剧情大纲并确认（用户明确要求直接写作时除外）；大纲要点可作为回复内容输出，正文仍只通过本工具写入。

"#;

/// 思维模式要求（写卡/纯聊两种模式共用）：注入 system prompt 的工具说明之后。
const THINKING_MODE_PROMPT: &str = r#"【思维模式要求】在你的思考过程（<think>标签内）中，请遵守以下规则：
1. 禁止使用圆括号包裹内心独白，例如"（心想：……）"或"（内心OS：……）"，所有分析内容直接陈述即可
2. 禁止以角色第一人称描写内心活动，例如"我心想""我觉得""我暗自"等，请用分析性语言替代
3. 思考内容应聚焦于剧情走向分析和回复内容规划，不要在思考中进行角色扮演式的内心戏表演
4. 每次思考必须以 `<｜begin▁of▁thinking｜>嗯，我现在要仔细分析当前情境、角色状态和用户输入，然后构思如何发展符合角色定义的剧情。`开头，以 `<｜end▁of▁thinking｜>` 结尾。"#;

/// 纯聊模式 system prompt：游戏模式（RPG）——AI 担任叙事者并控制所有 NPC，
/// 用户作为玩家直接行动/说话，剧情在聊天中即时演出；不提供任何工具。
/// 主角即用户，涉及主角的称呼与描写一律使用第二人称"你"；
/// 正文只描写玩家的有限视角（不展示 NPC 心理等背面内容）、无大纲确认、
/// 抉择权归玩家（禁止代替玩家做决定）、玩家输入中【】括起内容视为 Author's Note。
/// 注入结构：<参考资料>/<创作准则>/<剧情概览>（{OVERVIEW} 运行时替换为 overview.md 内容）。
const CHAT_TASK_PROMPT: &str = r#"这是一个互动角色扮演游戏（RPG）。你担任叙事者并控制故事中的所有 NPC，为用户打造沉浸式的游戏体验。

【角色分工】
- 你：叙事者 + 所有 NPC。叙事者负责场景、环境、氛围与事件的描写和推进；NPC 依据角色设定拥有各自的性格，通过台词、动作等外显言行与玩家互动
- 用户：玩家，也是故事的主角。用户的输入可以是行动描述或台词，你以叙事者和 NPC 的身份直接回应并推进剧情；涉及主角的称呼与描写一律使用第二人称"你"

【游戏规则】
- 剧情由角色性格 + 玩家输入驱动：玩家输入触发剧情，但剧情的展开必须严格符合角色设定与世界观，由角色性格自然地推动剧情走向，不得预设或强制推进特定剧情走向
- 不输出剧情大纲、不等待确认、不做剧情规划说明——每一轮回复都是剧情的直接延续

【视角与抉择】
- 正文只描写玩家的有限视角：你的所见所闻、环境与 NPC 的外显言行（主角始终以第二人称"你"称呼与描写）
- NPC 的心理、意图、动机等背面内容不向玩家展示，由你在思考中把握，不写入正文
- 剧情推进到需要玩家行动或抉择时，正文即结束，交由玩家行动
- 除非玩家明确要求，否则禁止代替玩家做出任何决定

【Author's Note】
- 玩家输入中用【】括起的内容视为 Author's Note（作者注）——玩家以作者身份提供的元指导（如节奏、风格、剧情走向、世界观补充等）
- 遵从其指示执行，但不视为游戏内台词，且不改变视角与抉择规则

【先澄清再演出】
- 遇到需要澄清的地方（玩家输入有歧义、信息不足、无法确定剧情该如何推进、或可能误读玩家意图时），先向玩家提出澄清问题，待玩家澄清后再生成正文
- 不猜测、不擅自假设、不基于模糊输入强行推进剧情；澄清问题本身不算正文

【输出格式要求】
- 回复内容即演出本身：叙事者的描写（场景、动作、氛围）与 NPC 的台词、动作交织展开，用连贯叙事直接输出，不通过任何工具写入（本会话不提供工具）
- 不加章节标题、分节标题、"第一章/第二章"之类的结构标记；正文中不混入大纲、梗概、写作思路、剧情说明或任何与正文无关的文字
- 每次根据玩家输入推进一段剧情，保持角色性格与创作准则（文风）一致

下面是一些参考资料和互动历史，请仔细阅读：
<参考资料>
{REFERENCE}
</参考资料>
<创作准则>
{GUIDELINES}
</创作准则>
<剧情概览>
{OVERVIEW}
</剧情概览>
{TOOL_AND_THINKING}
<互动历史>
"#;

/// 写作模式工具集：仅 4 个剧情卡片工具（素材沉淀由回复后的后台提取任务负责）。
fn creation_tool_definitions() -> Vec<ChatCompletionTool> {
    vec![
        ChatCompletionTool {
            r#type: ChatCompletionToolType::Function,
            function: FunctionObject {
                name: "read_story_cards".into(),
                description: Some(
                    "读取本故事已保存的全部剧情卡片（按轮次从早到晚），用于回顾已写剧情、保持故事连续性。仅在需要回顾前文时调用，不要每轮重复调用。"
                        .into(),
                ),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "last_n": {
                            "type": "integer",
                            "description": "可选：只返回最近 N 张卡片（按轮次倒序）；不传则返回全部"
                        }
                    }
                })),
                strict: None,
            },
        },
        ChatCompletionTool {
            r#type: ChatCompletionToolType::Function,
            function: FunctionObject {
                name: "read_story_card".into(),
                description: Some(
                    "按轮次读取单张剧情卡片或其片段。当用户消息中出现 [第N轮] 引用标记时，传 round=N 读取整卡；出现 [第N轮:起始-结束] 标记时，传 round=N 与 start/end 读取该片段（聚焦该片段处理，如续写、修改）；旧格式 [卡片:第N轮](card-xxx) 时传 card_id 读取该卡片。round 与 card_id 至少提供一个。读取结果返回卡片 id，供 update_story_card 使用。"
                        .into(),
                ),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "round": {
                            "type": "integer",
                            "description": "轮次 N（[第N轮] / [第N轮:起始-结束] 标签中的 N）"
                        },
                        "card_id": {
                            "type": "string",
                            "description": "卡片 ID（旧格式 [卡片:第N轮](card-xxx) 引用标记链接目标中的 card-xxx）"
                        },
                        "start": {
                            "type": "integer",
                            "description": "可选：片段起始字符位置（含）；提供时按 [start, end) 截取片段返回"
                        },
                        "end": {
                            "type": "integer",
                            "description": "可选：片段结束字符位置（不含）"
                        }
                    }
                })),
                strict: None,
            },
        },
        ChatCompletionTool {
            r#type: ChatCompletionToolType::Function,
            function: FunctionObject {
                name: "update_story_card".into(),
                description: Some(
                    "按卡片 ID 更新剧情卡片内容。用户要求修改已保存的剧情时使用：先调用 read_story_card 读取原内容，修改后通过本工具写回；更新后的正文仍只通过工具写入，回复中不输出正文。"
                        .into(),
                ),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "card_id": {
                            "type": "string",
                            "description": "要更新的卡片 ID"
                        },
                        "content": {
                            "type": "string",
                            "description": "更新后的剧情正文（仅正文本身，不含章节标题、大纲或说明文字）"
                        }
                    },
                    "required": ["card_id", "content"]
                })),
                strict: None,
            },
        },
        ChatCompletionTool {
            r#type: ChatCompletionToolType::Function,
            function: FunctionObject {
                name: "save_story_card".into(),
                description: Some(
                    "每完成一段完整剧情正文后调用本工具，将剧情正文写入创作卡片；正文只通过本工具写入，回复内容中不要直接输出正文。content 只包含剧情正文，不含章节标题或说明文字。澄清提问、讨论分析阶段不得调用。"
                        .into(),
                ),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "content": {
                            "type": "string",
                            "description": "剧情正文（仅正文本身，不含章节标题、大纲或说明文字）"
                        }
                    },
                    "required": ["content"]
                })),
                strict: None,
            },
        },
    ]
}

pub const CREATION_SUMMARY_PROMPT: &str = r#"你是一个小说创作对话的总结助手。请用简洁的中文总结以下创作对话，按以下格式输出，空字段省略不写。

## 故事概览
故事核心设定

## 人物状态
当前出场角色及其位置/状态/关系变化

## 情节进展
按时间线列出最近关键事件

## 待展开线索
已埋下但未回收的伏笔或用户表示"后面要写"的内容

## 当前位置
故事停在哪个场景、谁在做什么
"#;

fn gen_msg_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("msg-{}-{}", ts, COUNTER.fetch_add(1, Ordering::Relaxed))
}

#[derive(Debug, Serialize, Clone)]
struct ChatEvent {
    #[serde(rename = "type")]
    event_type: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    phase: Option<String>,
}

fn emit_event(app: &AppHandle, event_type: &str, content: &str) {
    let event = ChatEvent {
        event_type: event_type.into(),
        content: content.into(),
        phase: None,
    };
    app.emit("chat-event", event).ok();
}

fn emit_phase_event(app: &AppHandle, event_type: &str, content: &str, phase: &str) {
    let event = ChatEvent {
        event_type: event_type.into(),
        content: content.into(),
        phase: Some(phase.into()),
    };
    app.emit("chat-event", event).ok();
}

/// 历史消息行（含工具调用记录），用于跨 Turn 上下文重建。
#[derive(Debug, Clone)]
pub struct HistoryRow {
    pub role: String,
    pub content: String,
    pub reasoning: Option<String>,
    /// API 形状的 tool_calls JSON 数组字符串（assistant 工具轮保存时写入）
    pub tool_calls: Option<String>,
    pub tool_call_id: Option<String>,
}

/// 加载会话历史用于上下文回传。`phase` 为 Some 时按消息阶段过滤。
/// tool 消息也加载（重建工具轮需要）；reasoning 列可能为 NULL（用户消息）。
pub fn load_history(
    session_id: &str,
    phase: Option<&str>,
) -> Result<Vec<HistoryRow>, AppError> {
    db::with_db(|conn| {
        let rows: Vec<HistoryRow> = if let Some(p) = phase {
            let mut stmt = conn.prepare(
                "SELECT role, content, reasoning, tool_calls, tool_call_id FROM story_messages \
                 WHERE session_id = ?1 AND phase = ?2 AND summarized = 0 \
                 ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map(rusqlite::params![session_id, p], |row| {
                Ok(HistoryRow {
                    role: row.get(0)?,
                    content: row.get(1)?,
                    reasoning: row.get(2)?,
                    tool_calls: row.get(3)?,
                    tool_call_id: row.get(4)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        } else {
            let mut stmt = conn.prepare(
                "SELECT role, content, reasoning, tool_calls, tool_call_id FROM story_messages \
                 WHERE session_id = ?1 AND summarized = 0 \
                 ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map(rusqlite::params![session_id], |row| {
                Ok(HistoryRow {
                    role: row.get(0)?,
                    content: row.get(1)?,
                    reasoning: row.get(2)?,
                    tool_calls: row.get(3)?,
                    tool_call_id: row.get(4)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        Ok(rows)
    })
    .map_err(AppError::Database)
}

/// 将历史行按正常消息格式追加到 API messages，完全采用 BitFun 的跨 Turn 策略：
/// - 工具轮完整重建：assistant（tool_calls + 完整 reasoning_content）+ tool 消息配对回传
/// - reasoning_content 完整保留（含空串），让 DeepSeek 能验证原始 assistant turn 形态
/// - content 为空但带 tool_calls 的 assistant：content 省略（OpenAI 规范允许）
/// - content 为空且无 tool_calls 的 assistant：跳过（协议不合法）
/// - 孤儿 tool 消息（无前置 assistant tool_calls）：跳过
pub fn append_history_to_messages(
    messages: &mut Vec<serde_json::Value>,
    history_rows: &[HistoryRow],
) {
    use std::collections::HashSet;
    // 当前 assistant 轮期望回传的 tool_call_id 集合
    let mut expected_tool_ids: HashSet<String> = HashSet::new();
    for row in history_rows {
        if row.role == "assistant" {
            let tool_calls: Vec<serde_json::Value> = row
                .tool_calls
                .as_deref()
                .map(|s| serde_json::from_str(s).unwrap_or_default())
                .unwrap_or_default();
            let has_tool_calls = !tool_calls.is_empty();
            let content_empty = row.content.trim().is_empty();

            if content_empty && !has_tool_calls {
                // 纯工具轮但缺少 tool_calls 记录（旧数据），无法合法重建，跳过
                expected_tool_ids.clear();
                continue;
            }

            let mut pa = serde_json::json!({"role": "assistant"});
            if !content_empty {
                pa["content"] = serde_json::json!(row.content);
            }
            // reasoning_content 完整保留（DeepSeek 要求工具轮必须回传）
            if let Some(rc) = &row.reasoning {
                pa["reasoning_content"] = serde_json::json!(rc);
            }
            if has_tool_calls {
                expected_tool_ids = tool_calls
                    .iter()
                    .filter_map(|tc| tc["id"].as_str().map(str::to_string))
                    .collect();
                pa["tool_calls"] = serde_json::Value::Array(tool_calls);
            } else {
                expected_tool_ids.clear();
            }
            messages.push(pa);
        } else if row.role == "tool" {
            let Some(tool_call_id) = row.tool_call_id.clone().filter(|id| !id.is_empty()) else {
                continue;
            };
            // 只回传与当前 assistant 期望的 tool_call_id 匹配的结果
            if !expected_tool_ids.remove(&tool_call_id) {
                continue;
            }
            messages.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": tool_call_id,
                "content": row.content,
            }));
        } else {
            expected_tool_ids.clear();
            messages.push(serde_json::json!({"role": "user", "content": row.content}));
        }
    }
}

/// 将流式工具调用转成 API 形状的 JSON 数组字符串（用于持久化 tool_calls 列）。
pub fn tool_calls_to_json(tool_calls: &[crate::deepseek::ToolCall]) -> String {
    let arr: Vec<serde_json::Value> = tool_calls
        .iter()
        .map(|tc| {
            serde_json::json!({
                "id": tc.id,
                "type": "function",
                "function": {"name": tc.name, "arguments": tc.arguments}
            })
        })
        .collect();
    serde_json::to_string(&arr).unwrap_or_default()
}

/// 故事模式：card（写卡，正文沉淀为剧情卡片）/ chat（纯聊，正文直接回复在聊天框）。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StoryMode {
    Card,
    Chat,
}

/// 写作模式（Phase 2）：角色性格驱动、用户主导方向的创意写作。
/// 按故事模式分流：写卡模式暴露 4 个剧情卡片工具、正文写卡；
/// 纯聊模式不提供任何工具、正文直接在回复中输出，且不自动触发素材提取。
async fn handle_creation_phase(
    app: &AppHandle,
    client: &reqwest::Client,
    api_key: &str,
    session_id: &str,
    story_id: &str,
    message: &str,
    model: &str,
    mode: StoryMode,
    cancel: Arc<AtomicBool>,
) -> Result<String, AppError> {
    // Load history: creation phase only, tool messages excluded.
    // reasoning column may be NULL (user messages), so read as Option.
    let history_rows = load_history(session_id, Some("creation"))?;

    println!("[chat] creation history loaded: {} messages", history_rows.len());

    // Load session summary
    let summary: Option<String> = db::with_db(|conn| {
        conn.query_row(
            "SELECT summary FROM story_sessions WHERE id = ?1",
            [session_id],
            |row| row.get(0),
        )
    })
    .map_err(AppError::Database)
    .ok();

    let summary_text = match &summary {
        Some(s) if !s.trim().is_empty() => {
            format!("\n<历史摘要>\n以下是之前对话的关键信息摘要，请据此继续保持故事连贯性：\n{}\n</历史摘要>\n", s)
        }
        _ => String::new(),
    };

    // System prompt: task rules + reference.md + 创作准则（含齁叫呻吟） + 工具说明/思维模式 + <互动历史> opening tag
    // 纯聊模式额外注入 <剧情概览>（overview.md，充当无卡片场景下的连续性锚点）
    let reference = materials::read_story_md(story_id, "reference").unwrap_or_default();
    let reference_block = if reference.trim().is_empty() {
        "（暂无参考资料）".to_string()
    } else {
        reference
    };
    let guidelines = materials::read_story_md(story_id, "guidelines").unwrap_or_default();
    let guidelines_block = if guidelines.trim().is_empty() {
        "（暂无创作准则）".to_string()
    } else {
        guidelines
    };
    let overview_block = if mode == StoryMode::Chat {
        let overview = materials::read_story_md(story_id, "overview").unwrap_or_default();
        if overview.trim().is_empty() {
            "（暂无剧情概览，请依据当前对话与创作准则续写）".to_string()
        } else {
            overview
        }
    } else {
        String::new()
    };
    // 写卡模式：工具说明 + 思维模式要求；纯聊模式：仅思维模式要求（无工具）
    let tool_and_thinking = match mode {
        StoryMode::Card => format!("{}{}", CREATION_TOOL_DESCRIPTION, THINKING_MODE_PROMPT),
        StoryMode::Chat => THINKING_MODE_PROMPT.to_string(),
    };
    let task_prompt = match mode {
        StoryMode::Card => CREATION_TASK_PROMPT,
        StoryMode::Chat => CHAT_TASK_PROMPT,
    };
    let system_prompt = format!(
        "{}{}",
        summary_text,
        task_prompt
            .replace("{REFERENCE}", &reference_block)
            .replace("{GUIDELINES}", &guidelines_block)
            .replace("{OVERVIEW}", &overview_block)
            .replace("{TOOL_AND_THINKING}", &tool_and_thinking),
    );

    let mut messages: Vec<serde_json::Value> = Vec::new();
    messages.push(serde_json::json!({"role": "system", "content": system_prompt}));

    // History: normal message format (no tool_calls reconstruction);
    // empty assistant messages (pure save_story_card tool-call rounds) are skipped
    append_history_to_messages(&mut messages, &history_rows);

    // Latest user message: <最新互动> + closing </互动历史> only
    let user_msg = format!(
        "<最新互动>\n{}\n</最新互动>\n</互动历史>",
        message,
    );
    messages.push(serde_json::json!({"role": "user", "content": user_msg}));

    // 写卡模式暴露 4 个剧情卡片工具；纯聊模式不提供任何工具（正文直接在回复中输出）
    let tools = match mode {
        StoryMode::Card => creation_tool_definitions(),
        StoryMode::Chat => Vec::new(),
    };
    let mut full_content = String::new();
    let session_id_owned = session_id.to_string();
    let story_id_owned = story_id.to_string();
    // 本轮是否产出或修改过剧情卡片（决定回复后是否触发后台素材提取）
    let mut story_modified_this_turn = false;
    let max_turns = 200;

    for _turn in 0..max_turns {
        if cancel.load(Ordering::SeqCst) { break; }

        let app_clone = app.clone();
        let cancel_clone = cancel.clone();
        let result = deepseek::stream_chat(client, api_key, model, &messages, &tools, |event| {
            match event {
                StreamEvent::Reasoning(text) => emit_phase_event(&app_clone, "reasoning", &text, "creation"),
                StreamEvent::Text(text) => emit_phase_event(&app_clone, "text", &text, "creation"),
                StreamEvent::ToolCallStart { index, name } => {
                    emit_phase_event(&app_clone, "tool_call_start", &serde_json::json!({
                        "index": index,
                        "name": &name,
                        "args": {},
                    }).to_string(), "creation");
                },
                StreamEvent::Done => {},
            }
        }, cancel_clone)
        .await;

        match result {
            Ok(r) => {
                if cancel.load(Ordering::SeqCst) { break; }

                if r.tool_calls.is_empty() {
                    let content = r.content.clone();
                    let reasoning = r.reasoning.clone();
                    if !content.is_empty() || !reasoning.is_empty() {
                        let msg_id = gen_msg_id();
                        let _ = db::with_db(|conn| {
                            conn.execute(
                                "INSERT INTO story_messages (id, session_id, role, content, reasoning, tool_calls, phase) VALUES (?1, ?2, 'assistant', ?3, ?4, '', 'creation')",
                                rusqlite::params![msg_id, &session_id_owned, &content, &reasoning],
                            )
                        });
                    }
                    full_content = r.content;
                    maybe_summarize_after(client, api_key, &session_id_owned, r.usage.as_ref()).await;

                    // 写卡模式：回复完成且本轮产出或修改过剧情卡片 → 后台自动触发素材提取；
                    // 纯聊模式：不自动触发，由用户手动点击「素材沉淀」按钮触发。
                    if mode == StoryMode::Card && story_modified_this_turn {
                        let app_owned = app.clone();
                        let client_owned = client.clone();
                        let api_key_owned = api_key.to_string();
                        let story_id_ext = story_id_owned.clone();
                        let session_id_ext = session_id_owned.clone();
                        tokio::spawn(async move {
                            let _ = materials::run_material_extraction(
                                &app_owned, &client_owned, &api_key_owned, &story_id_ext, &session_id_ext,
                            )
                            .await;
                        });
                    }
                    break;
                }

                // Save intermediate assistant message (with tool_calls for history replay)
                if !r.content.is_empty() || !r.reasoning.is_empty() {
                    let c = r.content.clone();
                    let re = r.reasoning.clone();
                    let tcs = tool_calls_to_json(&r.tool_calls);
                    let msg_id = gen_msg_id();
                    let _ = db::with_db(|conn| {
                        conn.execute(
                            "INSERT INTO story_messages (id, session_id, role, content, reasoning, tool_calls, phase) VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, 'creation')",
                            rusqlite::params![msg_id, &session_id_owned, &c, &re, &tcs],
                        )
                    });
                }

                // Execute tool calls
                for (i, tc) in r.tool_calls.iter().enumerate() {
                    let args: serde_json::Value = serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!({}));

                    emit_phase_event(app, "tool_execute_start", &serde_json::json!({
                        "index": i,
                        "name": &tc.name,
                        "args": &args,
                    }).to_string(), "creation");

                    let tool_result = execute_creation_tool(app, &tc.name, &args, &story_id_owned, &session_id_owned).await;
                    let result_obj: serde_json::Value = serde_json::from_str(&tool_result).unwrap_or(serde_json::json!({"raw": tool_result}));

                    // 本轮保存或更新过剧情卡片 → 回复完成后触发后台素材提取
                    if (tc.name == "save_story_card" || tc.name == "update_story_card")
                        && !tool_result.contains("\"error\"")
                    {
                        story_modified_this_turn = true;
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

                    emit_phase_event(app, "tool_call_end", &serde_json::json!({
                        "index": i,
                        "name": &tc.name,
                        "result": &result_obj,
                    }).to_string(), "creation");

                    // Save tool result (no dedup needed in creation mode)
                    let tool_msg_id = gen_msg_id();
                    let _ = db::with_db(|conn| {
                        conn.execute(
                            "INSERT INTO story_messages (id, session_id, role, content, tool_call_id, phase) VALUES (?1, ?2, 'tool', ?3, ?4, 'creation')",
                            rusqlite::params![tool_msg_id, session_id_owned,
                                serde_json::json!({"name": &tc.name, "result": &result_obj}).to_string(),
                                &tc.id,
                            ],
                        )
                    });

                    // Append to API messages
                    let mut assistant_msg = serde_json::json!({
                        "role": "assistant",
                        "content": r.content.clone(),
                        "tool_calls": [{"id": &tc.id, "type": "function", "function": {"name": &tc.name, "arguments": &tc.arguments}}],
                    });
                    // DeepSeek thinking mode requires reasoning_content to be passed back (preserve even empty)
                    assistant_msg["reasoning_content"] = serde_json::json!(r.reasoning);
                    messages.push(assistant_msg);
                    messages.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": &tc.id,
                        "content": tool_result,
                    }));
                }
            }
            Err(e) => {
                if cancel.load(Ordering::SeqCst) { break; }
                emit_phase_event(app, "text", &format!("AI 请求失败: {}", e), "creation");
                emit_phase_event(app, "done", "", "creation");
                return Err(AppError::AiApi(e));
            }
        }
    }

    emit_phase_event(app, "done", "", "creation");
    Ok(full_content)
}

/// Execute a creation-mode tool call: read_story_cards (review) + save_story_card (save).
async fn execute_creation_tool(
    app: &AppHandle,
    name: &str,
    args: &serde_json::Value,
    story_id: &str,
    session_id: &str,
) -> String {
    match name {
        "read_story_cards" => {
            match story_cards::get_story_cards(story_id.to_string()) {
                Ok(cards) => {
                    let last_n = args.get("last_n").and_then(|v| v.as_i64());
                    let mut cards = cards;
                    if let Some(n) = last_n {
                        let n = n.max(0) as usize;
                        if n < cards.len() {
                            cards = cards[cards.len() - n..].to_vec();
                        }
                    }
                    let arr: Vec<serde_json::Value> = cards
                        .iter()
                        .map(|c| serde_json::json!({"id": c.id, "round": c.round_number, "content": c.content}))
                        .collect();
                    serde_json::json!({"cards": arr, "count": arr.len()}).to_string()
                }
                Err(e) => format!(r#"{{"error":"{}"}}"#, e),
            }
        }
        "read_story_card" => {
            let round = args.get("round").and_then(|v| v.as_i64());
            let card_id = args.get("card_id").and_then(|v| v.as_str()).unwrap_or("");
            let card = if let Some(r) = round {
                story_cards::get_story_card_by_round(story_id.to_string(), r as i32)
            } else if !card_id.is_empty() {
                story_cards::get_story_card(card_id.to_string())
            } else {
                return r#"{"error":"round 与 card_id 至少提供一个"}"#.into();
            };
            match card {
                Ok(c) => {
                    let start = args.get("start").and_then(|v| v.as_i64()).unwrap_or(0).max(0) as usize;
                    let end = args.get("end").and_then(|v| v.as_i64()).unwrap_or(0).max(0) as usize;
                    let has_range = args.get("start").is_some() || args.get("end").is_some();
                    if has_range {
                        // 按字符区间截取片段（start/end clamp 到内容长度，end 不小于 start）
                        let chars: Vec<char> = c.content.chars().collect();
                        let len = chars.len();
                        let s = start.min(len);
                        let e = end.min(len).max(s);
                        let fragment: String = chars[s..e].iter().collect();
                        serde_json::json!({
                            "id": c.id,
                            "round": c.round_number,
                            "start": s,
                            "end": e,
                            "content": fragment,
                        })
                        .to_string()
                    } else {
                        serde_json::json!({
                            "id": c.id,
                            "round": c.round_number,
                            "content": c.content,
                        })
                        .to_string()
                    }
                }
                Err(e) => format!(r#"{{"error":"{}"}}"#, e),
            }
        }
        "update_story_card" => {
            let card_id = args.get("card_id").and_then(|v| v.as_str()).unwrap_or("");
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            if card_id.is_empty() {
                return r#"{"error":"card_id 不能为空"}"#.into();
            }
            if content.trim().is_empty() {
                return r#"{"error":"content 不能为空"}"#.into();
            }
            match story_cards::update_story_card(card_id.to_string(), content.to_string()) {
                Ok(card) => {
                    app.emit("card_saved", serde_json::json!({"storyId": story_id})).ok();
                    serde_json::json!({"cardId": card.id, "roundNumber": card.round_number, "result": "updated"}).to_string()
                }
                Err(e) => format!(r#"{{"error":"{}"}}"#, e),
            }
        }
        "save_story_card" => {
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            if content.trim().is_empty() {
                return r#"{"error":"content 不能为空"}"#.into();
            }
            match story_cards::create_story_card(story_id.to_string(), session_id.to_string(), content.to_string()) {
                Ok(card) => {
                    app.emit("card_saved", serde_json::json!({"storyId": story_id})).ok();
                    serde_json::json!({"cardId": card.id, "roundNumber": card.round_number, "result": "saved"}).to_string()
                }
                Err(e) => format!(r#"{{"error":"{}"}}"#, e),
            }
        }
        _ => format!(r#"{{"error":"Unknown tool: {}"}}"#, name),
    }
}

#[tauri::command]
pub async fn chat(
    app: AppHandle,
    session_id: String,
    message: String,
    model: Option<String>,
) -> Result<(), AppError> {
    let api_key = deepseek::get_api_key().map_err(|e| AppError::AiApi(e))?;

    // Look up the session: story_id + model + story mode (card/chat)
    let (story_id, session_model, story_mode): (String, String, String) = db::with_db(|conn| {
        conn.query_row(
            "SELECT s.story_id, s.model, st.mode FROM story_sessions s JOIN stories st ON st.id = s.story_id WHERE s.id = ?1",
            [&session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
    })
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound("会话不存在".into()),
        other => AppError::Database(other),
    })?;

    let model = model.unwrap_or(session_model);

    // 按故事模式分流：card（写卡）/ chat（纯聊）
    let mode = if story_mode == "chat" {
        StoryMode::Chat
    } else {
        StoryMode::Card
    };

    // Reset cancellation flag for this request
    let cancel_state = app.state::<CancelState>();
    cancel_state.0.store(false, Ordering::SeqCst);
    let cancel = cancel_state.0.clone();

    println!("[chat] model={model} session={session_id} msg_len={}", message.len());

    let client = reqwest::Client::new();

    // Writing mode: character-driven creative writing (card / chat per story mode)
    let _ = handle_creation_phase(
        &app, &client, &api_key, &session_id, &story_id, &message, &model, mode, cancel,
    )
    .await?;

    Ok(())
}

#[tauri::command]
pub fn stop_chat(app: AppHandle) {
    let cancel_state = app.state::<CancelState>();
    cancel_state.0.store(true, Ordering::SeqCst);
    println!("[chat] stop signal sent");
}

#[tauri::command]
pub async fn summarize_session(
    app: AppHandle,
    session_id: String,
) -> Result<String, AppError> {
    let api_key = deepseek::get_api_key().map_err(|e| AppError::AiApi(e))?;

    // Get non-summarized messages
    let history: Vec<(String, String)> = db::with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT role, content FROM story_messages WHERE session_id = ?1 AND summarized = 0 AND role != 'tool' ORDER BY created_at ASC"
        )?;
        let rows = stmt.query_map([&session_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect()
    })
    .map_err(AppError::Database)?;

    if history.is_empty() {
        return Ok("没有需要总结的内容".into());
    }

    let conversation: String = history
        .iter()
        .map(|(role, content)| format!("{}: {}", role, content))
        .collect::<Vec<_>>()
        .join("\n\n");

    let messages = vec![
        serde_json::json!({
            "role": "system",
            "content": CREATION_SUMMARY_PROMPT
        }),
        serde_json::json!({
            "role": "user",
            "content": format!("请总结以下对话：\n\n{}", conversation)
        }),
    ];

    let client = reqwest::Client::new();
    let app_clone = app.clone();
    let result = deepseek::stream_chat(&client, &api_key, "deepseek-v4-flash", &messages, &[], move |event| {
        if let StreamEvent::Text(text) = event {
            emit_event(&app_clone, "text", &text);
        }
    }, Arc::new(AtomicBool::new(false)))
    .await;

    let summary = match result {
        Ok(r) => r.content,
        Err(e) => return Err(AppError::AiApi(format!("总结失败: {}", e))),
    };

    // Mark messages as summarized
    let _ = db::with_db(|conn| {
        conn.execute(
            "UPDATE story_messages SET summarized = 1 WHERE session_id = ?1 AND summarized = 0",
            [&session_id],
        )?;
        conn.execute(
            "UPDATE story_sessions SET summary = ?1 WHERE id = ?2",
            rusqlite::params![summary, session_id],
        )?;
        Ok::<_, rusqlite::Error>(())
    });

    emit_event(&app, "done", "");
    Ok(summary)
}

/// Check API usage after a chat response and auto-compress if approaching the context limit.
/// Uses the BitFun-style context compressor (see compression module): it keeps the recent
/// tail and key user messages verbatim, and summarizes the rest.
async fn maybe_summarize_after(
    client: &reqwest::Client,
    api_key: &str,
    session_id: &str,
    usage: Option<&crate::deepseek::Usage>,
) {
    const TOTAL_TOKENS_THRESHOLD: u64 = 96000; // 75% of 128K
    if let Some(u) = usage {
        if u.total_tokens > TOTAL_TOKENS_THRESHOLD {
            let _ = crate::compression::execute_compression(client, api_key, session_id).await;
        }
    }
}
