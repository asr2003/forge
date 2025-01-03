use derive_more::derive::Display;
use forge_tool::{ToolDefinition, ToolName};
use serde::{Deserialize, Serialize};

use super::response::{FunctionCall, OpenRouterToolCall};
use crate::{CompletionMessage, ModelId, Request, Role, ToolCallId};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TextContent {
    // TODO: could be an enum
    pub r#type: String,
    pub text: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ImageContentPart {
    pub r#type: String,
    pub image_url: ImageUrl,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OpenRouterMessage {
    pub role: OpenRouterRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<ToolName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<ToolCallId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenRouterToolCall>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl MessageContent {
    pub fn cached(self) -> Self {
        match self {
            MessageContent::Text(text) => MessageContent::Parts(vec![ContentPart::Text {
                text,
                cache_control: Some(CacheControl { type_: CacheControlType::Ephemeral }),
            }]),
            _ => self,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    ImageUrl {
        image_url: ImageUrl,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub type_: CacheControlType,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum CacheControlType {
    Ephemeral,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FunctionDescription {
    pub description: Option<String>,
    pub name: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OpenRouterTool {
    // TODO: should be an enum
    pub r#type: String,
    pub function: FunctionDescription,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum ToolChoice {
    None,
    Auto,
    Function { name: String },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ResponseFormat {
    pub r#type: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Prediction {
    pub r#type: String,
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProviderPreferences {
    // Define fields as necessary
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OpenRouterRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<OpenRouterMessage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    pub model: ModelId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OpenRouterTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repetition_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<std::collections::HashMap<u32, f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_a: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prediction: Option<Prediction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transforms: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderPreferences>,
}

impl From<ToolDefinition> for OpenRouterTool {
    fn from(value: ToolDefinition) -> Self {
        OpenRouterTool {
            r#type: "function".to_string(),
            function: FunctionDescription {
                description: Some(value.description),
                name: value.name.into_string(),
                parameters: serde_json::to_value(value.input_schema).unwrap(),
            },
        }
    }
}

impl From<Request> for OpenRouterRequest {
    fn from(request: Request) -> Self {
        OpenRouterRequest {
            messages: {
                let messages = request
                    .messages
                    .into_iter()
                    .map(OpenRouterMessage::from)
                    .collect::<Vec<_>>();

                Some(insert_cache(messages))
            },
            tools: {
                let tools = request
                    .tools
                    .into_iter()
                    .map(OpenRouterTool::from)
                    .collect::<Vec<_>>();
                if tools.is_empty() {
                    None
                } else {
                    Some(tools)
                }
            },
            model: request.model,
            prompt: Default::default(),
            response_format: Default::default(),
            stop: Default::default(),
            stream: Default::default(),
            max_tokens: Default::default(),
            temperature: Default::default(),
            tool_choice: Default::default(),
            seed: Default::default(),
            top_p: Default::default(),
            top_k: Default::default(),
            frequency_penalty: Default::default(),
            presence_penalty: Default::default(),
            repetition_penalty: Default::default(),
            logit_bias: Default::default(),
            top_logprobs: Default::default(),
            min_p: Default::default(),
            top_a: Default::default(),
            prediction: Default::default(),
            transforms: Default::default(),
            models: Default::default(),
            route: Default::default(),
            provider: Default::default(),
        }
    }
}

impl From<CompletionMessage> for OpenRouterMessage {
    fn from(value: CompletionMessage) -> Self {
        match value {
            CompletionMessage::ContentMessage(chat_message) => OpenRouterMessage {
                role: chat_message.role.into(),
                content: Some(MessageContent::Text(chat_message.content)),
                name: None,
                tool_call_id: None,
                tool_calls: chat_message.tool_call.map(|tool_call| {
                    // FIXME: All the tool_calls should be added, instead of just one of them
                    vec![OpenRouterToolCall {
                        id: tool_call.call_id,
                        r#type: "function".to_string(),
                        function: FunctionCall {
                            arguments: serde_json::to_string(&tool_call.arguments).unwrap(),
                            name: Some(tool_call.name),
                        },
                    }]
                }),
            },
            CompletionMessage::ToolMessage(tool_result) => OpenRouterMessage {
                role: OpenRouterRole::Tool,
                content: Some(MessageContent::Text(
                    serde_json::to_string(&tool_result.content).unwrap(),
                )),
                name: Some(tool_result.name),
                tool_call_id: tool_result.call_id,
                tool_calls: None,
            },
        }
    }
}

/// Inserts cache control information into system messages
/// NOTE: We need to add more caching as the context grows larger
fn insert_cache(mut message: Vec<OpenRouterMessage>) -> Vec<OpenRouterMessage> {
    for message in message.iter_mut() {
        if message.role == OpenRouterRole::System {
            message.content = message.content.clone().map(|a| a.cached());
        }
    }

    message
}

impl From<Role> for OpenRouterRole {
    fn from(role: Role) -> Self {
        match role {
            Role::System => OpenRouterRole::System,
            Role::User => OpenRouterRole::User,
            Role::Assistant => OpenRouterRole::Assistant,
        }
    }
}

#[derive(Debug, Deserialize, Display, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpenRouterRole {
    System,
    User,
    Assistant,
    Tool,
}
