use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::openai::RefinementRequest;

#[derive(Clone)]
pub struct GroqLLMClient {
    client: Client,
    base_url: String,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatContent,
}

#[derive(Deserialize)]
struct ChatContent {
    content: String,
}

impl GroqLLMClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .build()
            .context("Failed to build HTTP client for Groq LLM")?;
        let base_url = "https://api.groq.com/openai".to_string();
        Ok(Self { client, base_url })
    }

    pub async fn refine_transcript(&self, text: String, job: &RefinementRequest) -> Result<String> {
        if text.trim().is_empty() {
            return Ok(String::new());
        }

        if job.api_key.trim().is_empty() {
            return Err(anyhow!("Groq API key is required for post-processing"));
        }

        let Some(system_prompt) = job.system_prompt() else {
            return Ok(text);
        };

        let url = format!(
            "{}/v1/chat/completions",
            self.base_url.trim_end_matches('/')
        );

        let request = ChatRequest {
            model: "openai/gpt-oss-20b".to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: text.trim().to_string(),
                },
            ],
            temperature: 0.3,
        };

        let response = self
            .client
            .post(url)
            .bearer_auth(&job.api_key)
            .json(&request)
            .send()
            .await
            .context("Groq LLM refinement request failed")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<failed to read error body>".into());
            return Err(anyhow!(
                "Groq LLM responded with {} to refinement request: {}",
                status,
                body
            ));
        }

        let payload: ChatResponse = response
            .json()
            .await
            .context("Failed to parse Groq LLM refinement response")?;

        payload
            .choices
            .first()
            .map(|choice| choice.message.content.trim().to_string())
            .ok_or_else(|| anyhow!("Groq LLM refinement response contained no choices"))
    }
}
