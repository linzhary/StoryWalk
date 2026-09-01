use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use crate::db;
use crate::deepseek::{self, ChatCompletionTool, FunctionObject};
use async_openai::types::ChatCompletionToolType;
use tauri::{AppHandle, Emitter};

fn gen_msg_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("msg-{}-{}", ts, COUNTER.fetch_add(1, Ordering::Relaxed))
}

// ---- Tool definitions ----

pub fn material_tool_definitions() -> Vec<ChatCompletionTool> {
    vec![
        ChatCompletionTool {
            r#type: ChatCompletionToolType::Function,
            function: FunctionObject {
                name: "read_story_md".into(),
                description: Some("读取指定 MD 素材文件的完整内容".into()),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "file": {
                            "type": "string",
                            "enum": ["reference", "guidelines", "overview"],
                            "description": "要读取的文件：reference（相关资料）、guidelines（创作准则）或 overview（当前剧情概览）"
                        }
                    },
                    "required": ["file"]
                })),
                strict: None,
            },
        },
        ChatCompletionTool {
            r#type: ChatCompletionToolType::Function,
            function: FunctionObject {
                name: "patch_story_md".into(),
                description: Some(
                    "增量编辑 MD 文件：查找 old_str 并替换为 new_str。old_str 必须在文件中精确匹配且唯一出现。\
                     用于增删改特定章节或段落，避免重写整个文件。"
                    .into(),
                ),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "file": {
                            "type": "string",
                            "enum": ["reference", "guidelines", "overview"],
                            "description": "要编辑的文件"
                        },
                        "old_str": {
                            "type": "string",
                            "description": "要被替换的原文片段，必须在文件中唯一精确匹配"
                        },
                        "new_str": {
                            "type": "string",
                            "description": "替换后的新文本"
                        }
                    },
                    "required": ["file", "old_str", "new_str"]
                })),
                strict: None,
            },
        },
        ChatCompletionTool {
            r#type: ChatCompletionToolType::Function,
            function: FunctionObject {
                name: "update_story_md".into(),
                description: Some("全量重写 MD 文件。仅在文件为空或需要大规模重构时使用，日常优先使用 patch_story_md。".into()),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "file": {
                            "type": "string",
                            "enum": ["reference", "guidelines", "overview"],
                            "description": "要重写的文件"
                        },
                        "content": {
                            "type": "string",
                            "description": "完整的文件新内容（Markdown 格式）"
                        }
                    },
                    "required": ["file", "content"]
                })),
                strict: None,
            },
        },
    ]
}

// ---- File system helpers ----

fn get_story_dir(story_id: &str) -> PathBuf {
    let base = if cfg!(debug_assertions) {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    } else {
        // In production, use the app data dir via db module's data_dir concept.
        // Since db.rs uses the same logic, we mirror it here.
        // We use a simple heuristic: the data.db parent directory.
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };
    base.join("stories").join(story_id)
}

fn get_md_path(story_id: &str, file: &str) -> PathBuf {
    get_story_dir(story_id).join(format!("{}.md", file))
}

pub fn init_story_dir(story_id: &str) -> std::io::Result<()> {
    let dir = get_story_dir(story_id);
    fs::create_dir_all(&dir)?;
    // Create empty MD files if they don't exist
    // overview.md（当前剧情概览）仅纯聊模式使用；写卡故事也会创建空文件兜底，但不会注入/维护
    for f in &["reference", "guidelines", "overview"] {
        let path = dir.join(format!("{}.md", f));
        if !path.exists() {
            fs::write(&path, "")?;
        }
    }
    Ok(())
}

pub fn delete_story_dir(story_id: &str) -> std::io::Result<()> {
    let dir = get_story_dir(story_id);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

/// 构建创建故事时的 guidelines.md 初始内容（现代/古代文风种子）。
/// 按 style 选择现代/古代容器文件与对应文风的对白库：
/// 将 `<写作文风>` 内占位 `<补充规则/>`、`<自省约束/>` 替换为通用规则/约束内容，
/// 将 `</写作文风>` 之后的 `<示例对白/>` 替换为按文风选择的对白库（预摘录文件，不做运行时处理），
/// 最后拼接固定的初始创作准则段（# 创作准则）。
pub fn build_guidelines_seed(style: Option<&str>) -> String {
    let main = match style {
        Some("ancient") => crate::chat::CREATION_STYLE_ANCIENT,
        _ => crate::chat::CREATION_STYLE_MODERN,
    };
    let dialogue = match style {
        Some("ancient") => crate::chat::CREATION_STYLE_ANCIENT_DIALOGUE,
        _ => crate::chat::CREATION_STYLE_MODERN_DIALOGUE,
    };
    let mut seed = main
        .replace("<补充规则/>", crate::chat::CREATION_STYLE_RULES.trim())
        .replace("<自省约束/>", crate::chat::CREATION_STYLE_SELF_CHECK.trim())
        .replace("<示例对白/>", dialogue.trim())
        .trim()
        .to_string();
    seed.push_str("\n\n");
    seed.push_str(crate::chat::CREATION_INITIAL_GUIDELINES.trim());
    seed
}

pub fn read_story_md(story_id: &str, file: &str) -> Result<String, String> {
    let path = get_md_path(story_id, file);
    if !path.exists() {
        return Ok(String::new());
    }
    fs::read_to_string(&path).map_err(|e| format!("读取文件失败: {}", e))
}

pub fn update_story_md(story_id: &str, file: &str, content: &str) -> Result<(), String> {
    let dir = get_story_dir(story_id);
    fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {}", e))?;
    let path = dir.join(format!("{}.md", file));
    fs::write(&path, content).map_err(|e| format!("写入文件失败: {}", e))
}

pub fn patch_story_md(story_id: &str, file: &str, old_str: &str, new_str: &str) -> Result<(), String> {
    if old_str.is_empty() {
        return Err("old_str 不能为空".into());
    }
    let current = read_story_md(story_id, file)?;
    let count = current.matches(old_str).count();
    if count == 0 {
        return Err(format!("未找到匹配的文本片段。请确认 old_str 内容是否正确。文件当前内容前200字: {}", 
            &current.chars().take(200).collect::<String>()));
    }
    if count > 1 {
        return Err(format!("old_str 匹配到 {} 处，不唯一。请提供更长的上下文使其唯一匹配。", count));
    }
    let updated = current.replacen(old_str, new_str, 1);
    update_story_md(story_id, file, &updated)
}

// ---- Tool execution ----

pub async fn execute_material_tool(name: &str, args: &serde_json::Value, story_id: &str) -> String {
    match name {
        "read_story_md" => {
            let file = args.get("file").and_then(|v| v.as_str()).unwrap_or("");
            if file.is_empty() {
                return r#"{"error":"Missing required param: file"}"#.into();
            }
            match read_story_md(story_id, file) {
                Ok(content) => serde_json::json!({"file": file, "content": content}).to_string(),
                Err(e) => format!(r#"{{"error":"{}"}}"#, e),
            }
        }
        "patch_story_md" => {
            let file = args.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let old_str = args.get("old_str").and_then(|v| v.as_str()).unwrap_or("");
            let new_str = args.get("new_str").and_then(|v| v.as_str()).unwrap_or("");
            if file.is_empty() {
                return r#"{"error":"Missing required param: file"}"#.into();
            }
            match patch_story_md(story_id, file, old_str, new_str) {
                Ok(()) => serde_json::json!({"file": file, "result": "patched"}).to_string(),
                Err(e) => format!(r#"{{"error":"{}"}}"#, e),
            }
        }
        "update_story_md" => {
            let file = args.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            if file.is_empty() {
                return r#"{"error":"Missing required param: file"}"#.into();
            }
            match update_story_md(story_id, file, content) {
                Ok(()) => serde_json::json!({"file": file, "result": "updated"}).to_string(),
                Err(e) => format!(r#"{{"error":"{}"}}"#, e),
            }
        }
        _ => format!(r#"{{"error":"Unknown tool: {}"}}"#, name),
    }
}

// ---- 后台素材提取（回复完成后由写作会话触发） ----

pub const EXTRACTION_PROMPT: &str = r#"你是故事素材沉淀助手。你的任务是从写作对话中识别新引入的设定、剧情进展与文风偏好，简洁地沉淀到两个素材文件：
- reference.md（相关资料）：角色设定、世界观、故事背景等全局上下文，以及每轮剧情进展/定稿摘要
- guidelines.md（创作准则）：写作风格、叙事规则、禁止句式等即时约束（禁写入剧情内容）

【文件结构须知】
reference.md 结构：
- 顶部为设定区：角色、世界观、背景等全局设定
- ## 剧情进展 章节：按轮次记录剧情进展/定稿摘要（时间线式）

guidelines.md 由以下部分组成：
- <写作文风>…</写作文风>：系统生成的文风规范（内部含 <词汇系统>、<人设铁律>、<角色差异化反应>、<反差羞辱张力>、<外貌描写>、<拟声词系统>、<场景铺垫与氛围>、<对白与描写交织> 等子区块）
- <示例对白>…</示例对白>：系统生成的对白临摹库
- # 创作准则 及之后的章节：故事专属的创作准则（可沉淀、可追加）

【保护系统标签——非必要不改】
- <写作文风> 与 <示例对白> 标签及其内部所有内容，是系统生成的规范化素材，沉淀时禁止修改：不 patch、不 update、不重写
- 新沉淀的内容一律追加到 <写作文风> 之外的故事专属区域（# 创作准则 章节或新增章节）
- 仅当用户明确要求变更文风、规则或对白风格时，才允许在用户指明的最小范围内修改对应区块

【工作流程】
1. 先调用 read_story_md 读取 reference.md 和 guidelines.md，了解已沉淀内容
2. 对照写作对话，识别其中新引入的、素材中尚未记录的信息：
   - 新角色、地点、世界观设定 → reference.md（设定区）
   - 写作风格偏好、叙事规则、风格反馈 → guidelines.md（追加到 # 创作准则 区域）
   - 剧情进展、轮次定稿 → reference.md（追加到「剧情进展」章节）
3. 使用 patch_story_md 增量更新（优先）；仅在文件为空或需大改时用 update_story_md——全量重写时必须原样保留 <写作文风>、<示例对白> 标签及其内部内容，只增改故事专属区域

【简洁沉淀约束】
- 只沉淀明确的新设定/风格偏好，用短条目记录关键信息（如"黄蓉——九阴真经驻颜，40 岁容貌 20+"），不写长篇说明
- 单次沉淀新增内容控制在几行以内；禁止复制整段剧情、对话原文或已有内容
- 已存在的内容不重复写入
- 信息模糊、与现有素材冲突、或不确定归属时不猜测：跳过该条，在回复中简要说明
- 若没有值得沉淀的新信息，直接回复"无新素材"并结束，不要写入任何内容

【去主语化记录——只关注内容，不关注提出方】
- 沉淀条目直接记录信息本身，禁止带主语：不写"用户提出/用户要求/用户反馈/用户希望/用户觉得/用户让……"等转述式表述
- 条目形态是设定事实或风格规则本身（如"黄蓉——九阴真经驻颜，40 岁容貌 20+""禁止'啊'字堆砌呻吟"），而不是对话过程描述（如"用户希望角色更傲娇"应记为"角色性格：傲娇，口嫌体正直"）
- 只关注内容的正确性与必要性，不记录"谁在什么时候提出了什么"
- 回复文本同样避免"用户xxx"表述：直接说明沉淀了哪些条目，或回复"无新素材"

【剧情进展沉淀——归属 reference.md，禁止写入 guidelines.md】
- 每轮写作完成后的剧情进展/定稿，以简洁时间线条目沉淀到 reference.md 的「剧情进展」章节（如“### 第N轮” + 关键剧情节点、伏笔状态、角色状态变化），禁复制剧情正文
- 剧情进展、轮次定稿、剧情大纲禁止写入 guidelines.md——guidelines.md 只放可复用的写作风格/叙事规则/禁止句式，剧情内容不属于创作准则
- 带「第N轮」锚定的条目分类处理：跨轮通用规则 → 去轮次锚定后写入 guidelines.md；单轮剧情定稿/进展 → reference.md「剧情进展」章节

【当前剧情概览——归属 overview.md（仅纯聊模式维护）】
- 纯聊模式的故事才需要维护 overview.md（当前剧情概览）；写卡模式不写 overview.md
- 概览是一份「现在时」的剧情快照，与 reference.md 的「剧情进展」历史时间线职责不同：进展是逐轮累积的记录，概览只呈现当前最新状态，二者都写，互不替代
- 概览内容用简洁条目组织：当前位置/场景（故事停在哪个场景、谁在做什么）、当前出场角色及状态、关键设定要点、未回收伏笔、下一步待展开线索
- 每次沉淀时用 update_story_md 全量重写 overview.md：只保留最新状态，覆盖掉已过期的旧条目，不追加历史、不复制剧情正文
- 若尚无值得写入的剧情内容（如仅讨论阶段），概览保持原样或写入最简占位

【一致性检查——写作文风】
- <写作文风> 区块是当前生效的文风规范，沉淀的内容不得重复描述其中已有的规则、词汇、句式、描写要求
- 若待沉淀内容与 <写作文风> 已覆盖的规范相同或类似，视为已有约束，跳过不写
- 仅沉淀 <写作文风> 之外的故事专属设定与具体风格反馈

【patch_story_md 使用技巧】
- 新增章节：old_str 取文件最后一个标题行或结尾空行，new_str = "\n\n## 新章节\n内容"
- 追加内容到已有章节：old_str 取该章节最后一段，new_str = old_str + "\n\n新内容"
- 修改段落：old_str 精确匹配要修改的段落；确保 old_str 唯一——标题名唯一时用包含标题的较长片段

【工具失败处理】
- patch_story_md 返回 error（未找到/匹配多处）时：先 read_story_md 获取精确文本，修正 old_str 后重试；同一工具连续失败 2 次后停止，向用户说明原因

请用中文回复。"#;

/// 后台素材提取任务：写作回复完成后调用。
/// 懒创建/复用 mode='extraction' 的隐藏会话；system 换 EXTRACTION_PROMPT，
/// 写作会话完整历史按正常消息格式重放回传，末尾追加沉淀指导消息。
/// 提取记录（user/assistant/tool）落库到隐藏会话（phase='extraction'），不回传写作会话。
pub async fn run_material_extraction(
    app: &AppHandle,
    client: &reqwest::Client,
    api_key: &str,
    story_id: &str,
    writing_session_id: &str,
) -> Result<(), String> {
    app.emit("material_extraction", serde_json::json!({"storyId": story_id, "status": "start"})).ok();

    // 查询故事模式：纯聊（chat）模式额外维护 overview.md（当前剧情概览），写卡模式不写
    let story_mode: String = db::with_db(|conn| {
        conn.query_row(
            "SELECT mode FROM stories WHERE id = ?1",
            [story_id],
            |row| row.get(0),
        )
    })
    .unwrap_or_else(|_| "card".into());

    // 懒创建/复用隐藏提取会话
    let extraction_session_id: String = db::with_db(|conn| {
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM story_sessions WHERE story_id = ?1 AND mode = 'extraction' LIMIT 1",
                [story_id],
                |row| row.get(0),
            )
            .ok();
        if let Some(id) = existing {
            return Ok(id);
        }
        let id = db::gen_id("session");
        conn.execute(
            "INSERT INTO story_sessions (id, story_id, title, mode, model) VALUES (?1, ?2, ?3, 'extraction', 'deepseek-v4-flash')",
            rusqlite::params![id, story_id, "素材沉淀（隐藏）"],
        )?;
        Ok(id)
    })
    .map_err(|e| format!("创建提取会话失败: {}", e))?;

    // 1. system 换成素材提取 prompt
    let mut messages: Vec<serde_json::Value> = Vec::new();
    messages.push(serde_json::json!({
        "role": "system",
        "content": EXTRACTION_PROMPT,
    }));

    // 2. 写作会话完整历史重放（含用户消息、AI 剧情正文、工具结果）
    let history_rows = crate::chat::load_history(writing_session_id, None)
        .map_err(|e| format!("加载写作历史失败: {}", e))?;
    crate::chat::append_history_to_messages(&mut messages, &history_rows);

    // 3. 末尾沉淀指导消息（纯聊模式追加 overview.md 当前剧情概览的维护指令）
    let guide_message = if story_mode == "chat" {
        "请基于以上完整的写作对话执行素材沉淀：重点分析最近一轮（最新一条消息及 AI 的剧情回复）引入的新信息，对照整个对话历史与素材文件，按系统提示的规则执行沉淀；剧情进展/轮次定稿沉淀到 reference.md 的「剧情进展」章节，不写入 guidelines.md；本故事为纯聊模式，请另外先 read overview.md（当前剧情概览），沉淀完成后用 update_story_md 全量重写 overview.md，只保留当前最新状态（当前位置/场景、在场角色及状态、关键设定、未回收伏笔、下一步待展开线索），覆盖已过期条目，不复制剧情正文；若没有值得沉淀的新信息，直接回复\"无新素材\"结束，不要写入任何内容。"
    } else {
        "请基于以上完整的写作对话执行素材沉淀：重点分析最近一轮（最新一条消息及 AI 的剧情回复）引入的新信息，对照整个对话历史与素材文件，按系统提示的规则执行沉淀；剧情进展/轮次定稿沉淀到 reference.md 的「剧情进展」章节，不写入 guidelines.md；若没有值得沉淀的新信息，直接回复\"无新素材\"结束，不要写入任何内容。"
    };
    messages.push(serde_json::json!({
        "role": "user",
        "content": guide_message,
    }));

    let tools = material_tool_definitions();
    let max_turns = 10;
    let session_id_owned = extraction_session_id;

    for _turn in 0..max_turns {
        let stream_result = deepseek::stream_chat(
            client,
            api_key,
            "deepseek-v4-flash",
            &messages,
            &tools,
            |_| {},
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .await;

        let result = match stream_result {
            Ok(r) => r,
            Err(e) => {
                println!("[extraction] failed: story={} err={}", story_id, e);
                app.emit("material_extraction", serde_json::json!({"storyId": story_id, "status": "failed"})).ok();
                return Err(e);
            }
        };

        if result.tool_calls.is_empty() {
            // 保存最终 assistant 消息
            if !result.content.is_empty() || !result.reasoning.is_empty() {
                let msg_id = gen_msg_id();
                let _ = db::with_db(|conn| {
                    conn.execute(
                        "INSERT INTO story_messages (id, session_id, role, content, reasoning, phase) VALUES (?1, ?2, 'assistant', ?3, ?4, 'extraction')",
                        rusqlite::params![msg_id, session_id_owned, result.content, result.reasoning],
                    )
                });
            }
            break;
        }

        // 保存中间 assistant 消息（带 tool_calls 供记录）
        if !result.content.is_empty() || !result.reasoning.is_empty() {
            let msg_id = gen_msg_id();
            let tcs = crate::chat::tool_calls_to_json(&result.tool_calls);
            let _ = db::with_db(|conn| {
                conn.execute(
                    "INSERT INTO story_messages (id, session_id, role, content, reasoning, tool_calls, phase) VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, 'extraction')",
                    rusqlite::params![msg_id, session_id_owned, result.content, result.reasoning, tcs],
                )
            });
        }

        // 执行工具调用
        for tc in &result.tool_calls {
            let args: serde_json::Value = serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!({}));
            let tool_result = execute_material_tool(&tc.name, &args, story_id).await;
            let result_obj: serde_json::Value = serde_json::from_str(&tool_result).unwrap_or(serde_json::json!({"raw": tool_result}));

            // 保存 tool 消息
            let tool_msg_id = gen_msg_id();
            let _ = db::with_db(|conn| {
                conn.execute(
                    "INSERT INTO story_messages (id, session_id, role, content, tool_call_id, phase) VALUES (?1, ?2, 'tool', ?3, ?4, 'extraction')",
                    rusqlite::params![tool_msg_id, session_id_owned,
                        serde_json::json!({"name": &tc.name, "result": &result_obj}).to_string(),
                        &tc.id,
                    ],
                )
            });

            // 追加到 API messages 供下一轮
            let assistant_msg = serde_json::json!({
                "role": "assistant",
                "content": result.content.clone(),
                "reasoning_content": result.reasoning,
                "tool_calls": [{
                    "id": &tc.id,
                    "type": "function",
                    "function": {"name": &tc.name, "arguments": &tc.arguments},
                }],
            });
            messages.push(assistant_msg);
            messages.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": &tc.id,
                "content": tool_result,
            }));
        }
    }

    app.emit("material_extraction", serde_json::json!({"storyId": story_id, "status": "done"})).ok();
    println!("[extraction] done: story={}", story_id);
    Ok(())
}

// ---- Tauri commands for direct frontend access ----

#[tauri::command]
pub fn read_story_materials(story_id: String, file: String) -> Result<String, String> {
    read_story_md(&story_id, &file)
}

#[tauri::command]
pub fn update_story_materials(story_id: String, file: String, content: String) -> Result<(), String> {
    update_story_md(&story_id, &file, &content)
}

/// 纯聊模式手动触发素材沉淀：复用后台提取任务（重放写作会话历史 → 沉淀到素材文件）。
/// 写卡模式由回复完成后的自动逻辑触发，无需前端调用。
#[tauri::command]
pub async fn trigger_material_extraction(
    app: AppHandle,
    story_id: String,
    session_id: String,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let api_key = crate::deepseek::get_api_key()?;
    run_material_extraction(&app, &client, &api_key, &story_id, &session_id).await
}
