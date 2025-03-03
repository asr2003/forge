mod agent;
mod chat_request;
mod chat_response;
mod context;
mod conversation;
mod env;
mod error;
mod event;
mod file;
mod message;
mod model;
mod orch;
mod point;
mod provider;
mod suggestion;
mod summarize;
mod template;
mod tool;
mod tool_call;
mod tool_call_parser;
mod tool_choice;
mod tool_definition;
mod tool_name;
mod tool_result;
mod tool_usage;
mod workflow;

pub use agent::*;
pub use chat_request::*;
pub use chat_response::*;
pub use context::*;
pub use conversation::*;
pub use env::*;
pub use error::*;
pub use event::*;
pub use file::*;
pub use message::*;
pub use model::*;
pub use orch::*;
pub use point::*;
pub use provider::*;
pub use suggestion::*;
pub use summarize::*;
pub use template::*;
pub use tool::*;
pub use tool_call::*;
pub use tool_call_parser::*;
pub use tool_choice::*;
pub use tool_definition::*;
pub use tool_name::*;
pub use tool_result::*;
pub use tool_usage::*;
pub use workflow::*;

#[async_trait::async_trait]
pub trait ProviderService: Send + Sync + 'static {
    async fn chat(
        &self,
        id: &ModelId,
        context: Context,
    ) -> ResultStream<ChatCompletionMessage, anyhow::Error>;
    async fn models(&self) -> anyhow::Result<Vec<Model>>;
    async fn parameters(&self, model: &ModelId) -> anyhow::Result<Parameters>;
}

#[async_trait::async_trait]
pub trait ToolService: Send + Sync {
    // TODO: should take `call` by reference
    async fn call(&self, call: ToolCallFull) -> ToolResult;
    fn list(&self) -> Vec<ToolDefinition>;
    fn usage_prompt(&self) -> String;
}

#[async_trait::async_trait]
pub trait ConversationService: Send + Sync {
    async fn get(&self, id: &ConversationId) -> anyhow::Result<Option<Conversation>>;
    async fn create(&self, workflow: Workflow) -> anyhow::Result<ConversationId>;
    async fn inc_turn(&self, id: &ConversationId, agent: &AgentId) -> anyhow::Result<()>;
    async fn set_context(
        &self,
        id: &ConversationId,
        agent: &AgentId,
        context: Context,
    ) -> anyhow::Result<()>;
    async fn insert_event(
        &self,
        conversation_id: &ConversationId,
        event: Event,
    ) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
pub trait TemplateService: Send + Sync {
    async fn render_system(
        &self,
        agent: &Agent,
        prompt: &Template<SystemContext>,
    ) -> anyhow::Result<String>;

    async fn render_event(
        &self,
        agent: &Agent,
        prompt: &Template<EventContext>,
        event: &Event,
    ) -> anyhow::Result<String>;
}

/// Core app trait providing access to services and repositories.
/// This trait follows clean architecture principles for dependency management
/// and service/repository composition.
pub trait App: Send + Sync + 'static {
    type ToolService: ToolService;
    type ProviderService: ProviderService;
    type ConversationService: ConversationService;
    type TemplateService: TemplateService;

    fn tool_service(&self) -> &Self::ToolService;
    fn provider_service(&self) -> &Self::ProviderService;
    fn conversation_service(&self) -> &Self::ConversationService;
    fn template_service(&self) -> &Self::TemplateService;
}
