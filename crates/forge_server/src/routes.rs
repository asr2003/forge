use std::sync::Arc;

const SERVER_PORT: u16 = 8080;

use axum::extract::{Json, State};
use axum::response::sse::{Event, Sse};
use axum::response::Html;
use axum::routing::{get, post};
use axum::Router;
use forge_domain::{Context, Environment, Model, ModelId, ToolDefinition, UStream};
use serde::Serialize;
use tokio_stream::{Stream, StreamExt};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::context::ContextEngine;
use crate::{
    ChatRequest, Conversation, ConversationHistory, ConversationId, CreateSettingRequest, Errata,
    File, Result, RootAPIService, Service, Setting, SettingId,
};
use crate::{Result, Service};

pub struct API {
    api: Arc<dyn RootAPIService>,
    env: Environment,
}

impl API {
    pub async fn init() -> Result<Self> {
        tracing_subscriber::fmt().init();
        let env = Service::environment_service().get().await?;
        let api = Arc::new(Service::root_api_service(env.clone()));

        Ok(Self { api, env })
    }
}

async fn context_html_handler(
    State(state): State<Arc<dyn RootAPIService>>,
    axum::extract::Path(id): axum::extract::Path<ConversationId>,
) -> Html<String> {
    let context = state.context(id).await.unwrap();
    let engine = ContextEngine::new(context);
    Html(engine.render_html())
}

impl API {
    pub async fn run(self, content: String) -> Result<UStream<ChatResponse>> {
        let model = ModelId::from_env(&self.env);
        let chat = ChatRequest { content, model, conversation_id: None };
        self.api.chat(chat).await
    }

    pub async fn launch(self) -> Result<()> {
        // Setup HTTP server
        let app = Router::new()
            .route("/conversation", post(conversation_handler))
            .route("/completions", get(completions_handler))
            .route("/health", get(health_handler))
            .route("/tools", get(tools_handler))
            .route("/models", get(models_handler))
            .route("/context/{id}", get(context_handler))
            .route("/context/{id}/html", get(context_html_handler))
            .route("/conversations", get(conversations_handler))
            .route("/conversation/{id}", get(history_handler))
            .route("/settings/{id}", get(setting_by_id_handler))
            .route("/settings", post(create_setting_handler))
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods([
                        axum::http::Method::GET,
                        axum::http::Method::POST,
                        axum::http::Method::OPTIONS,
                    ])
                    .allow_headers([
                        axum::http::header::CONTENT_TYPE,
                        axum::http::header::AUTHORIZATION,
                    ]),
            )
            .with_state(self.api.clone());

        // Spawn HTTP server
        let server = tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{SERVER_PORT}"))
                .await
                .unwrap();
            info!("Server running on http://127.0.0.1:{SERVER_PORT}");
            axum::serve(listener, app).await.unwrap();
        });

        // Wait for server to complete (though it runs indefinitely)
        let _ = server.await;

        Ok(())
    }
}

async fn completions_handler(
    State(state): State<Arc<dyn RootAPIService>>,
) -> axum::Json<Vec<File>> {
    let files = state
        .completions()
        .await
        .expect("Failed to get completions");
    axum::Json(files)
}

#[axum::debug_handler]
async fn conversation_handler(
    State(state): State<Arc<dyn RootAPIService>>,
    Json(request): Json<ChatRequest>,
) -> Sse<impl Stream<Item = std::result::Result<Event, std::convert::Infallible>>> {
    let stream = state
        .chat(request)
        .await
        .expect("Engine failed to respond with a chat message");
    Sse::new(stream.map(|message| {
        let data = serde_json::to_string(&message).expect("Failed to serialize message");
        Ok(Event::default().data(data))
    }))
}

#[axum::debug_handler]
async fn tools_handler(State(state): State<Arc<dyn RootAPIService>>) -> Json<ToolResponse> {
    let tools = state.tools().await;
    Json(ToolResponse { tools })
}

async fn health_handler() -> axum::response::Response {
    axum::response::Response::builder()
        .status(200)
        .body(axum::body::Body::empty())
        .unwrap()
}

async fn models_handler(State(state): State<Arc<dyn RootAPIService>>) -> Json<ModelResponse> {
    let models = state.models().await.unwrap_or_default();
    Json(ModelResponse { models })
}

async fn conversations_handler(
    State(state): State<Arc<dyn RootAPIService>>,
) -> Json<ConversationsResponse> {
    let conversations = state.conversations().await.unwrap_or_default();
    Json(ConversationsResponse { conversations })
}

async fn history_handler(
    State(state): State<Arc<dyn RootAPIService>>,
    axum::extract::Path(id): axum::extract::Path<ConversationId>,
) -> Json<ConversationHistory> {
    Json(state.conversation(id).await.unwrap_or_default())
}

#[axum::debug_handler]
async fn context_handler(
    State(state): State<Arc<dyn RootAPIService>>,
    axum::extract::Path(id): axum::extract::Path<ConversationId>,
) -> Json<ContextResponse> {
    let context = state.context(id).await.unwrap();
    Json(ContextResponse { context })
}

#[axum::debug_handler]
async fn setting_by_id_handler(
    State(state): State<Arc<dyn RootAPIService>>,
    axum::extract::Path(setting_id): axum::extract::Path<SettingId>,
) -> Json<Setting> {
    let setting = state.setting_by_id(setting_id).await.unwrap();
    Json(setting)
}

#[axum::debug_handler]
async fn create_setting_handler(
    State(state): State<Arc<dyn RootAPIService>>,
    Json(request): Json<CreateSettingRequest>,
) -> Json<Setting> {
    let setting = state.create_setting(request).await.unwrap();
    Json(setting)
}

#[derive(Serialize)]
pub struct ContextResponse {
    context: Context,
}

#[derive(Serialize)]
pub struct ModelResponse {
    models: Vec<Model>,
}

#[derive(Serialize)]
pub struct ToolResponse {
    tools: Vec<ToolDefinition>,
}

#[derive(Serialize)]
pub struct ConversationsResponse {
    conversations: Vec<Conversation>,
}
