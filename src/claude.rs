use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum LogEntry {
    Summary {
        summary: String,
    },
    User {
        message: UserMessage,
        timestamp: String,
        cwd: Option<String>,
    },
    Assistant {
        message: AssistantMessage,
        timestamp: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub struct UserMessage {
    pub content: UserContent,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum UserContent {
    String(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Deserialize)]
pub struct AssistantMessage {
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    #[serde(other)]
    Other,
}

fn extract_text_from_blocks(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn extract_text_from_user(message: &UserMessage) -> String {
    match &message.content {
        UserContent::String(text) => text.clone(),
        UserContent::Blocks(blocks) => extract_text_from_blocks(blocks),
    }
}

pub fn extract_text_from_assistant(message: &AssistantMessage) -> String {
    extract_text_from_blocks(&message.content)
}
