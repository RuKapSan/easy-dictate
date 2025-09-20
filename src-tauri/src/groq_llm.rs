use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::openai::TranscriptionJob;

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
            .context("Не удалось создать HTTP клиент для Groq LLM")?;
        let base_url = "https://api.groq.com/openai".to_string();
        Ok(Self { client, base_url })
    }

    pub async fn refine_transcript(&self, text: String, job: &TranscriptionJob) -> Result<String> {
        if !job.auto_translate
            && !(job.use_custom_instructions && !job.custom_instructions.trim().is_empty())
        {
            return Ok(text);
        }

        let url = format!(
            "{}/v1/chat/completions",
            self.base_url.trim_end_matches('/')
        );

        let mut directives = Vec::new();
        if job.auto_translate {
            directives.push(format!(
                "Сначала переведи текст на {} язык, сохранив естественный стиль и смысл.",
                job.target_language
            ));
        } else {
            directives.push(
                "Сохрани язык исходного текста, если пользовательские инструкции не требуют иного."
                    .to_string(),
            );
        }

        let custom = job.custom_instructions.trim();
        if job.use_custom_instructions && !custom.is_empty() {
            directives.push(format!(
                "Затем выполни следующие пользовательские инструкции: {}",
                custom
            ));
        }

        directives.push(
            "Ответь только итоговым текстом без пояснений, кавычек и служебных сообщений."
                .to_string(),
        );

        let system_prompt = format!(
            "Ты помощник по постобработке диктовок. Выполни шаги последовательно. {}",
            directives.join(" ")
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
                    content: text,
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
            .context("Ошибка запроса обработки текста к Groq LLM")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<не удалось получить тело ответа>".into());
            return Err(anyhow!(
                "Groq LLM вернул ошибку при обработке текста {}: {}",
                status,
                body
            ));
        }

        let payload: ChatResponse = response
            .json()
            .await
            .context("Не удалось прочитать ответ обработки текста от Groq LLM")?;

        payload
            .choices
            .first()
            .map(|choice| choice.message.content.trim().to_string())
            .ok_or_else(|| anyhow!("Пустой ответ от Groq LLM API"))
    }
}