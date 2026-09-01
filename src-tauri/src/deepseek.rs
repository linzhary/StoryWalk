use serde::{Deserialize, Serialize};
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub use async_openai::types::{ChatCompletionTool, FunctionObject};

const DEEPSEEK_BASE: &str = "https://api.deepseek.com/v1";

// ---- Public types ----

#[derive(Debug, Clone, Serialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Reasoning(String),
    Text(String),
    ToolCallStart { index: usize, name: String },
    Done,
}

pub struct StreamResult {
    pub reasoning: String,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<Usage>,
}

// ---- SSE chunk types ----

#[derive(Debug, Deserialize)]
struct Delta {
    #[serde(default)]
    pub reasoning_content: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCallChunk>>,
}

#[derive(Debug, Deserialize)]
struct ToolCallChunk {
    pub index: usize,
    #[serde(default)]
    pub id: Option<String>,
    pub function: Option<FunctionChunk>,
}

#[derive(Debug, Deserialize)]
struct FunctionChunk {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    pub delta: Delta,
}

#[derive(Debug, Deserialize)]
struct UsageRaw {
    pub prompt_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct SseChunk {
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<UsageRaw>,
}

// ---- Main API ----

pub async fn stream_chat(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    messages: &[serde_json::Value],
    tools: &[ChatCompletionTool],
    event_sender: impl Fn(StreamEvent),
    cancel: Arc<AtomicBool>,
) -> Result<StreamResult, String> {
    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true,
        "reasoning_effort": "max"
    });

    if !tools.is_empty() {
        body["tools"] = serde_json::to_value(tools).map_err(|e| e.to_string())?;
    }

    println!("[deepseek] request — model={model} msgs={}", messages.len());

    let response = client
        .post(format!("{}/chat/completions", DEEPSEEK_BASE))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("API 请求失败: {}", e))?;

    println!("[deepseek] response status={}", response.status());

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        println!("[deepseek] error body: {}", text);
        return Err(format!("API 错误 {}: {}", status.as_u16(), text));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut full_reasoning = String::new();
    let mut full_content = String::new();
    let mut tool_call_acc: std::collections::BTreeMap<usize, ToolCall> = std::collections::BTreeMap::new();
    let mut tool_call_started: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut last_usage: Option<Usage> = None;

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::SeqCst) {
            return Err("已取消".into());
        }
        let chunk = chunk.map_err(|e| format!("流读取错误: {}", e))?;
        let text = String::from_utf8_lossy(&chunk);
        buffer.push_str(&text);

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim().to_string();
            buffer = buffer[pos + 1..].to_string();

            let data = line.strip_prefix("data: ").unwrap_or("");
            if data.is_empty() || data == "[DONE]" {
                continue;
            }

            let parsed: SseChunk = match serde_json::from_str(data) {
                Ok(p) => p,
                Err(_) => continue,
            };

            if let Some(ref u) = parsed.usage {
                last_usage = Some(Usage {
                    prompt_tokens: u.prompt_tokens,
                    total_tokens: u.total_tokens,
                });
            }

            for choice in &parsed.choices {
                if let Some(ref rc) = choice.delta.reasoning_content {
                    full_reasoning.push_str(rc);
                    event_sender(StreamEvent::Reasoning(rc.clone()));
                }
                if let Some(ref c) = choice.delta.content {
                    full_content.push_str(c);
                    event_sender(StreamEvent::Text(c.clone()));
                }
                if let Some(ref tcs) = choice.delta.tool_calls {
                    for tc in tcs {
                        let entry = tool_call_acc.entry(tc.index).or_insert_with(|| ToolCall {
                            id: String::new(),
                            name: String::new(),
                            arguments: String::new(),
                        });
                        if let Some(ref id) = tc.id {
                            entry.id = id.clone();
                        }
                        if let Some(ref func) = tc.function {
                            if let Some(ref name) = func.name {
                                let had_name = !entry.name.is_empty();
                                entry.name.push_str(name);
                                if !had_name && !entry.name.is_empty() && tool_call_started.insert(tc.index) {
                                    event_sender(StreamEvent::ToolCallStart { index: tc.index, name: entry.name.clone() });
                                }
                            }
                            if let Some(ref args) = func.arguments {
                                entry.arguments.push_str(args);
                            }
                        }
                    }
                }
            }
        }
    }

    event_sender(StreamEvent::Done);

    let tool_calls: Vec<ToolCall> = tool_call_acc
        .into_values()
        .filter(|tc| !tc.id.is_empty() && !tc.name.is_empty())
        .collect();

    println!("[deepseek] done — content_len={} reasoning_len={} tools={} usage={:?}",
        full_content.len(), full_reasoning.len(), tool_calls.len(), last_usage);

    Ok(StreamResult {
        reasoning: full_reasoning,
        content: full_content,
        tool_calls,
        usage: last_usage,
    })
}

pub fn get_api_key() -> Result<String, String> {
    env::var("DEEPSEEK_API_KEY").map_err(|_| "DEEPSEEK_API_KEY 环境变量未设置".into())
}
