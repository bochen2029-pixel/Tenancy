use anyhow::{anyhow, Result};
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

use crate::think_strip::{strip_think, ThinkStripper};

// Qwen3.5 emits <think>...</think> reasoning tokens by default.
// For Dave's voice we want plain replies, no chain-of-thought theatre.
// Flip to true if a future Dave needs reasoning surfaced.
const ENABLE_THINKING: bool = false;

#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: Delta,
}

#[derive(Debug, Deserialize, Default)]
struct Delta {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CompletionResponse {
    choices: Vec<CompletionChoice>,
}

#[derive(Debug, Deserialize)]
struct CompletionChoice {
    message: CompletionMessage,
}

#[derive(Debug, Deserialize)]
struct CompletionMessage {
    content: String,
}

pub struct LlamaClient {
    base_url: String,
    http: reqwest::Client,
}

impl LlamaClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(600))
            .build()
            .expect("reqwest client");
        Self {
            base_url: base_url.into(),
            http,
        }
    }

    pub async fn chat_stream<F>(&self, messages: Vec<ChatMessage>, mut on_token: F) -> Result<String>
    where
        F: FnMut(&str) + Send,
    {
        let body = json!({
            "model": "dave",
            "messages": messages,
            "stream": true,
            "temperature": 0.85,
            "top_p": 0.9,
            "top_k": 20,
            "repeat_penalty": 1.0,
            "presence_penalty": 1.5,
            "chat_template_kwargs": {
                "enable_thinking": ENABLE_THINKING
            }
        });

        let resp = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("llama-server returned {}: {}", status, text));
        }

        let mut stream = resp.bytes_stream().eventsource();
        let mut stripper = ThinkStripper::new();

        while let Some(event) = stream.next().await {
            let event = event?;
            if event.data == "[DONE]" {
                break;
            }
            let chunk: StreamChunk = match serde_json::from_str(&event.data) {
                Ok(c) => c,
                Err(_) => continue,
            };
            if let Some(choice) = chunk.choices.first() {
                if let Some(content) = &choice.delta.content {
                    if !content.is_empty() {
                        // Strip <think>...</think> blocks at the SDK boundary.
                        // Callbacks see only post-strip text; defense in depth
                        // for Qwen's chat template ignoring enable_thinking.
                        let visible = stripper.push(content);
                        if !visible.is_empty() {
                            on_token(&visible);
                        }
                    }
                }
            }
        }

        Ok(stripper.finalize())
    }

    pub async fn complete(
        &self,
        messages: Vec<ChatMessage>,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<String> {
        let body = json!({
            "model": "dave",
            "messages": messages,
            "stream": false,
            "temperature": temperature,
            "max_tokens": max_tokens,
            "top_p": 0.9,
            "top_k": 20,
            "chat_template_kwargs": {
                "enable_thinking": ENABLE_THINKING
            }
        });

        let resp = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("llama-server returned {}: {}", status, text));
        }

        let parsed: CompletionResponse = resp.json().await?;
        let raw = parsed
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();
        // Strip <think>...</think> at the SDK boundary. Same A7-pattern
        // defense as chat_stream.
        Ok(strip_think(&raw).trim().to_string())
    }
}
