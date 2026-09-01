use serde::{Deserialize, Serialize};

const EXA_MCP_URL: &str = "https://mcp.exa.ai/mcp";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub async fn execute_web_search(query: &str, num_results: Option<u32>) -> String {
    let query = query.trim();
    if query.is_empty() {
        return serde_json::json!({"error": "Missing query"}).to_string();
    }
    let num = num_results.unwrap_or(5).clamp(1, 10);

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "web_search_exa",
            "arguments": {
                "query": query,
                "numResults": num,
                "type": "auto",
                "livecrawl": "fallback",
                "contextMaxCharacters": 8000
            }
        }
    });

    let client = reqwest::Client::new();
    let response = match client
        .post(EXA_MCP_URL)
        .header("accept", "application/json, text/event-stream")
        .header("content-type", "application/json")
        .json(&body)
        .timeout(std::time::Duration::from_secs(25))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return serde_json::json!({"error": format!("搜索失败: {}", e)}).to_string();
        }
    };

    let text = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            return serde_json::json!({"error": format!("读取响应失败: {}", e)}).to_string();
        }
    };

    // Parse SSE response
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("data: ") {
            continue;
        }
        let data = &trimmed[6..];
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
            let content = parsed
                .get("result")
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_array());
            if let Some(content_blocks) = content {
                let text_content: String = content_blocks
                    .iter()
                    .filter(|c| {
                        c.get("type").map_or(false, |t| t == "text")
                            || c.get("kind").map_or(false, |k| k == "text")
                    })
                    .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n");

                if !text_content.trim().is_empty() {
                    let results = parse_exa_results(&text_content);
                    return serde_json::json!({
                        "query": query,
                        "results": results,
                        "result_count": results.len()
                    })
                    .to_string();
                }
            }
        }
    }

    serde_json::json!({"error": "未找到搜索结果", "query": query}).to_string()
}

fn parse_exa_results(raw: &str) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let blocks: Vec<&str> = raw.split('\n').collect::<Vec<_>>();

    // Re-group by Title: lines
    let mut current_title = String::new();
    let mut current_url = String::new();
    let mut current_text = String::new();
    let mut in_text = false;

    for line in &blocks {
        if let Some(title) = line.strip_prefix("Title: ") {
            // Save previous result
            if !current_title.is_empty() && !current_url.is_empty() {
                let snippet = clean_snippet(&current_text);
                results.push(SearchResult {
                    title: current_title.clone(),
                    url: current_url.clone(),
                    snippet,
                });
            }
            current_title = title.trim().to_string();
            current_url = String::new();
            current_text = String::new();
            in_text = false;
        } else if let Some(url) = line.strip_prefix("URL: ") {
            current_url = url.trim().to_string();
        } else if line.starts_with("Text: ") {
            in_text = true;
            current_text = line[6..].to_string();
        } else if in_text && !line.starts_with('#') {
            current_text.push('\n');
            current_text.push_str(line);
        }
    }

    // Save last result
    if !current_title.is_empty() && !current_url.is_empty() {
        let snippet = clean_snippet(&current_text);
        results.push(SearchResult {
            title: current_title,
            url: current_url,
            snippet,
        });
    }

    // Fallback if no structured results
    if results.is_empty() && !raw.trim().is_empty() {
        let snippet = raw.trim().chars().take(320).collect::<String>();
        results.push(SearchResult {
            title: "搜索结果".into(),
            url: String::new(),
            snippet,
        });
    }

    results
}

fn clean_snippet(text: &str) -> String {
    let cleaned: String = text
        .lines()
        .filter(|l| !l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if cleaned.len() > 320 {
        format!("{}...", &cleaned[..320])
    } else {
        cleaned
    }
}
