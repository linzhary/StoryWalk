use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("HTTP 错误: {0}")]
    Http(#[from] reqwest::Error),
    #[error("未找到: {0}")]
    NotFound(String),
    #[error("无效输入: {0}")]
    InvalidInput(String),
    #[error("AI API 错误: {0}")]
    AiApi(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
