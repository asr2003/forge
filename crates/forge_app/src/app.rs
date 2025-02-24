use std::sync::Arc;

use forge_domain::App;

use crate::conversation::ForgeConversationService;
use crate::provider::ForgeProviderService;
use crate::template::ForgeTemplateService;
use crate::tool_service::ForgeToolService;
use crate::Infrastructure;

pub struct ForgeApp<F> {
    infra: Arc<F>,
    _tool_service: ForgeToolService,
    _provider_service: ForgeProviderService,
    _conversation_service: ForgeConversationService,
    _prompt_service: ForgeTemplateService,
}

impl<F: Infrastructure> ForgeApp<F> {
    pub fn new(infra: Arc<F>) -> Self {
        Self {
            infra: infra.clone(),
            _tool_service: ForgeToolService::new(infra.clone()),
            _provider_service: ForgeProviderService::new(infra.clone()),
            _conversation_service: ForgeConversationService::new(),
            _prompt_service: ForgeTemplateService::new(),
        }
    }
}

impl<F: Infrastructure> App for ForgeApp<F> {
    type ToolService = ForgeToolService;
    type ProviderService = ForgeProviderService;
    type ConversationService = ForgeConversationService;
    type PromptService = ForgeTemplateService;

    fn tool_service(&self) -> &Self::ToolService {
        &self._tool_service
    }

    fn provider_service(&self) -> &Self::ProviderService {
        &self._provider_service
    }

    fn conversation_service(&self) -> &Self::ConversationService {
        &self._conversation_service
    }

    fn prompt_service(&self) -> &Self::PromptService {
        &self._prompt_service
    }
}

impl<F: Infrastructure> Infrastructure for ForgeApp<F> {
    type EnvironmentService = F::EnvironmentService;
    type FileReadService = F::FileReadService;
    type VectorIndex = F::VectorIndex;
    type EmbeddingService = F::EmbeddingService;

    fn environment_service(&self) -> &Self::EnvironmentService {
        self.infra.environment_service()
    }

    fn file_read_service(&self) -> &Self::FileReadService {
        self.infra.file_read_service()
    }

    fn vector_index(&self) -> &Self::VectorIndex {
        self.infra.vector_index()
    }

    fn embedding_service(&self) -> &Self::EmbeddingService {
        self.infra.embedding_service()
    }
}
