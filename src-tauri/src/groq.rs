use anyhow::{anyhow, Context, Result};
use reqwest::{multipart::Form, Client};
use serde::Deserialize;

use crate::openai::TranscriptionJob;

#[derive(Clone)]
pub struct GroqClient {
    client: Client,
    base_url: String,
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

impl GroqClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .build()
            .context("Не удалось создать HTTP клиент для Groq")?;
        let base_url = "https://api.groq.com/openai".to_string();
        Ok(Self { client, base_url })
    }

    pub async fn transcribe(&self, job: TranscriptionJob) -> Result<String> {
        if job.api_key.trim().is_empty() {
            return Err(anyhow!("Укажите API ключ Groq в настройках"));
        }

        let url = format!(
            "{}/v1/audio/transcriptions",
            self.base_url.trim_end_matches('/')
        );

        let part = reqwest::multipart::Part::bytes(job.audio_wav)
            .file_name("clip.wav")
            .mime_str("audio/wav")
            .context("Не удалось подготовить файл для отправки")?;

        let model = if job.model.starts_with("groq/") {
            job.model.strip_prefix("groq/").unwrap_or(&job.model).to_string()
        } else {
            "whisper-large-v3-turbo".to_string()
        };

        let form = Form::new()
            .text("model", model)
            .text("response_format", "json")
            .part("file", part);

        let response = self
            .client
            .post(url)
            .bearer_auth(job.api_key)
            .multipart(form)
            .send()
            .await
            .context("Ошибка запроса к Groq")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<не удалось получить тело ответа>".into());
            return Err(anyhow!("Groq вернул ошибку {}: {}", status, body));
        }

        let payload: TranscriptionResponse = response
            .json()
            .await
            .context("Не удалось прочитать ответ Groq")?;
        Ok(payload.text.trim().to_string())
    }
}