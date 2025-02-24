use std::env;

use anyhow::Context;
use forge_app::EmbeddingService;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    input: String,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

pub struct ForgeEmbeddingService {
    client: reqwest::Client,
    api_key: String,
}

impl Default for ForgeEmbeddingService {
    fn default() -> Self {
        Self::new()
    }
}

impl ForgeEmbeddingService {
    pub fn new() -> Self {
        let api_key = env::var("OPENAI_API_KEY").unwrap_or_default();
        let client = reqwest::Client::new();
        Self { client, api_key }
    }
}

#[async_trait::async_trait]
impl EmbeddingService for ForgeEmbeddingService {
    async fn embed(&self, sentence: &str) -> anyhow::Result<Vec<f32>> {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_key))
                .context("Failed to create auth header")?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let request = EmbeddingRequest {
            model: "text-embedding-ada-002".to_string(),
            input: sentence.to_string(),
        };

        let response: EmbeddingResponse = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .headers(headers)
            .json(&request)
            .send()
            .await
            .context("Failed to send request to OpenAI")?
            .json()
            .await
            .context("Failed to parse OpenAI response")?;

        let embeddings = response
            .data
            .into_iter()
            .flat_map(|data| data.embedding)
            .collect();

        Ok(embeddings)
    }
}
