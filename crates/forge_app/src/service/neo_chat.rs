use std::sync::Arc;
use forge_domain::{
    AgentMessage, ChatRequest, ChatResponse, ConversationRepository,
    Orchestrator, ProviderService, ToolService, Variables, Workflow,
    ResultStream,
};
use tokio::sync::Mutex;
use futures::StreamExt;
use crate::mpsc_stream::MpscStream;

use super::Service;

#[async_trait::async_trait]
pub trait ChatService: Send + Sync {
    async fn chat(
        &self,
        prompt: ChatRequest,
        workflow: Workflow,
    ) -> ResultStream<AgentMessage<ChatResponse>, anyhow::Error>;
}

impl Service {
    pub fn chat_service(
        provider: Arc<dyn ProviderService>,
        tool: Arc<dyn ToolService>,
        conversation: Arc<dyn ConversationRepository>,
    ) -> impl ChatService {
        Live::new(provider, tool, conversation)
    }
}

struct Live {
    provider: Arc<dyn ProviderService>,
    tool: Arc<dyn ToolService>,
    conversation: Arc<dyn ConversationRepository>,
}

impl Live {
    fn new(
        provider: Arc<dyn ProviderService>,
        tool: Arc<dyn ToolService>,
        conversation: Arc<dyn ConversationRepository>,
    ) -> Self {
        Self { provider, tool, conversation }
    }
}

#[async_trait::async_trait]
impl ChatService for Live {
    async fn chat(
        &self,
        prompt: ChatRequest,
        workflow: Workflow,
    ) -> ResultStream<AgentMessage<ChatResponse>, anyhow::Error> {
        let provider = self.provider.clone();
        let tool = self.tool.clone();
        let workflow = Arc::new(Mutex::new(workflow));
        let mut input = Variables::default();
        input.add("task", prompt.content);
        let input = Arc::new(input);

        let stream = MpscStream::spawn(move |tx| {
            let orch = Arc::new(
                Orchestrator::new(provider, tool)
                    .workflow(workflow)
                    .sender(tx),
            );
            let input = input.clone();
            
            async move {
                if let Err(e) = orch.execute(&input).await {
                    eprintln!("Orchestrator execution error: {}", e);
                }
            }
        });

        Ok(Box::pin(stream.map(Ok)))
    }
}
