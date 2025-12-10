use anyhow::{anyhow, Context, Result};
use reqwest::{multipart::Form, Client};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct TranscriptionRequest {
    pub api_key: String,
    pub model: String,
    pub audio_wav: Vec<u8>,
}

impl RefinementRequest {
    pub fn has_custom_instructions(&self) -> bool {
        self.custom_instructions
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
    }

    pub fn has_vocabulary(&self) -> bool {
        !self.vocabulary.is_empty()
    }

    pub fn requires_refinement(&self) -> bool {
        self.auto_translate || self.has_custom_instructions() || self.has_vocabulary()
    }

    pub fn system_prompt(&self) -> Option<String> {
        if !self.requires_refinement() {
            return None;
        }

        let mut directives = Vec::new();

        // Vocabulary correction directive (first, so terms are fixed before other processing)
        if self.has_vocabulary() {
            let terms = self.vocabulary.join(", ");
            directives.push(format!(
                "Fix any misspelled technical terms. The correct spellings are: {}. If you see similar-sounding words that should be these terms, replace them.",
                terms
            ));
        }

        if self.auto_translate {
            directives.push(format!(
                "Translate the transcript into {}, keeping the original intent and tone.",
                self.target_language
            ));
        } else if !self.has_vocabulary() {
            // Only add generic polish if not just doing vocabulary correction
            directives.push(
                "Polish the transcript and fix clear mistakes while keeping intent.".to_string(),
            );
        }

        if let Some(extra) = self
            .custom_instructions
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            directives.push(format!(
                "Follow the user's additional instructions: {}",
                extra
            ));
        }

        directives.push("Return only the updated transcript with no commentary.".to_string());

        Some(format!(
            "You are assisting with high quality speech transcription cleanup. {}",
            directives.join(" ")
        ))
    }
}
#[derive(Clone, Debug)]
pub struct RefinementRequest {
    pub api_key: String,
    pub model: String,
    pub auto_translate: bool,
    pub target_language: String,
    pub custom_instructions: Option<String>,
    pub vocabulary: Vec<String>,
}

#[derive(Clone)]
pub struct OpenAiClient {
    client: Client,
    base_url: String,
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
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

impl OpenAiClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .build()
            .context("Failed to build HTTP client for OpenAI")?;
        let base_url = std::env::var("OPENAI_BASE_URL")
            .ok()
            .unwrap_or_else(|| "https://api.openai.com".to_string());
        Ok(Self { client, base_url })
    }

    pub async fn transcribe(&self, job: TranscriptionRequest) -> Result<String> {
        if job.api_key.trim().is_empty() {
            return Err(anyhow!("OpenAI API key is missing"));
        }

        let url = format!(
            "{}/v1/audio/transcriptions",
            self.base_url.trim_end_matches('/')
        );
        let part = reqwest::multipart::Part::bytes(job.audio_wav)
            .file_name("clip.wav")
            .mime_str("audio/wav")
            .context("Failed to build multipart payload for transcription")?;

        let form = Form::new()
            .text("model", job.model)
            .text("response_format", "json")
            .part("file", part);

        let response = self
            .client
            .post(url)
            .bearer_auth(job.api_key)
            .multipart(form)
            .send()
            .await
            .context("OpenAI transcription request failed")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<failed to read error body>".into());
            return Err(anyhow!("OpenAI responded with {}: {}", status, body));
        }

        let payload: TranscriptionResponse = response
            .json()
            .await
            .context("Failed to parse OpenAI transcription response")?;
        Ok(payload.text.trim().to_string())
    }

    pub async fn refine_transcript(&self, text: String, job: &RefinementRequest) -> Result<String> {
        if text.trim().is_empty() {
            return Ok(String::new());
        }

        if job.api_key.trim().is_empty() {
            return Err(anyhow!("OpenAI API key is required for post-processing"));
        }

        let Some(system_prompt) = job.system_prompt() else {
            return Ok(text);
        };

        let url = format!(
            "{}/v1/chat/completions",
            self.base_url.trim_end_matches('/')
        );

        let request = ChatRequest {
            model: job.model.clone(),
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
            .context("OpenAI refinement request failed")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<failed to read error body>".into());
            return Err(anyhow!(
                "OpenAI responded with {} to refinement request: {}",
                status,
                body
            ));
        }

        let payload: ChatResponse = response
            .json()
            .await
            .context("Failed to parse OpenAI refinement response")?;

        payload
            .choices
            .first()
            .map(|choice| choice.message.content.trim().to_string())
            .ok_or_else(|| anyhow!("OpenAI refinement response contained no choices"))
    }
}
