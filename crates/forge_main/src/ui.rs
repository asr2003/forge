use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use base64::Engine;
use colored::Colorize;
use forge_api::{AgentMessage, Attachment, ChatRequest, ChatResponse, ConversationId, Model, Usage, 
    API};
use forge_display::TitleFormat;
use forge_tracker::EventKind;
use futures::TryFutureExt;
use lazy_static::lazy_static;
use tokio_stream::StreamExt;

use crate::cli::Cli;
use crate::console::CONSOLE;
use crate::info::Info;
use crate::input::{Console, PromptInput};
use crate::model::{Command, UserInput};
use crate::{banner, log};

lazy_static! {
    pub static ref TRACKER: forge_tracker::Tracker = forge_tracker::Tracker::default();
}

#[derive(Default)]
struct UIState {
    current_title: Option<String>,
    conversation_id: Option<ConversationId>,
    usage: Usage,
}

impl From<&UIState> for PromptInput {
    fn from(state: &UIState) -> Self {
        PromptInput::Update {
            title: state.current_title.clone(),
            usage: Some(state.usage.clone()),
        }
    }
}

pub struct UI<F> {
    state: UIState,
    api: Arc<F>,
    console: Console,
    cli: Cli,
    models: Option<Vec<Model>>,
    #[allow(dead_code)] // The guard is kept alive by being held in the struct
    _guard: tracing_appender::non_blocking::WorkerGuard,
}

impl<F: API> UI<F> {
    pub fn init(cli: Cli, api: Arc<F>) -> Result<Self> {
        // Parse CLI arguments first to get flags

        let env = api.environment();
        Ok(Self {
            state: Default::default(),
            api,
            console: Console::new(env.clone()),
            cli,
            models: None,
            _guard: log::init_tracing(env.clone())?,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        // Handle direct prompt if provided
        let prompt = self.cli.prompt.clone();
        if let Some(prompt) = prompt {
            // TODO: add --attach arg through which users can pass files
            self.chat(prompt, vec![]).await?;
            return Ok(());
        }

        // Display the banner in dimmed colors since we're in interactive mode
        banner::display()?;

        let mut attachments = vec![];

        // Get initial input from file or prompt
        let mut input = match &self.cli.command {
            Some(path) => self.console.upload(path).await?,
            None => self.console.prompt(None).await?,
        };

        loop {
            match input {
                Command::Dump => {
                    self.handle_dump().await?;
                    let prompt_input = Some((&self.state).into());
                    input = self.console.prompt(prompt_input).await?;
                    continue;
                }
                Command::New => {
                    banner::display()?;
                    self.state = Default::default();
                    input = self.console.prompt(None).await?;

                    continue;
                }
                Command::Info => {
                    let info =
                        Info::from(&self.api.environment()).extend(Info::from(&self.state.usage));

                    CONSOLE.writeln(info.to_string())?;

                    let prompt_input = Some((&self.state).into());
                    input = self.console.prompt(prompt_input).await?;
                    continue;
                }
                Command::Message(ref content) => {
                    let chat_result = self.chat(content.clone(), attachments).await;
                    attachments = vec![];
                    if let Err(err) = chat_result {
                        CONSOLE.writeln(
                            TitleFormat::failed(format!("{:?}", err))
                                .sub_title(self.state.usage.to_string())
                                .format(),
                        )?;
                    }
                    let prompt_input = Some((&self.state).into());
                    input = self.console.prompt(prompt_input).await?;
                }
                Command::Exit => {
                    break;
                }
                Command::Models => {
                    let models = if let Some(models) = self.models.as_ref() {
                        models
                    } else {
                        let models = self.api.models().await?;
                        self.models = Some(models);
                        self.models.as_ref().unwrap()
                    };
                    let info: Info = models.as_slice().into();
                    CONSOLE.writeln(info.to_string())?;

                    input = self.console.prompt(None).await?;
                }
                Command::Attach(paths) => {
                    if paths.is_empty() {
                        CONSOLE.writeln(
                            "Error: No file paths provided. Usage: /attach <file1> [file2 ...]",
                        )?;
                    } else {
                        // Validate files exist and are images
                        for path in &paths {
                            if !path.exists() {
                                CONSOLE.writeln(format!(
                                    "Error: File not found: {}",
                                    path.display()
                                ))?;
                            }
                        }
                        // TODO: somehow show the attachments from UI
                        let new_attachments = prepare_attachments(paths).await;
                        attachments.extend(new_attachments);
                    }
                    input = self.console.prompt(Some((&self.state).into())).await?;
                }
            }
        }

        Ok(())
    }

    async fn chat(&mut self, content: String, files: Vec<Attachment>) -> Result<()> {
        let conversation_id = match self.state.conversation_id {
            Some(ref id) => id.clone(),
            None => {
                let conversation_id = self
                    .api
                    .init(self.api.load(self.cli.workflow.as_deref()).await?)
                    .await?;
                self.state.conversation_id = Some(conversation_id.clone());

                conversation_id
            }
        };

        let chat = ChatRequest { content: content.clone(), conversation_id, files };

        tokio::spawn(TRACKER.dispatch(EventKind::Prompt(content)));
        match self.api.chat(chat).await {
            Ok(mut stream) => self.handle_chat_stream(&mut stream).await,
            Err(err) => Err(err),
        }
    }

    async fn handle_chat_stream(
        &mut self,
        stream: &mut (impl StreamExt<Item = Result<AgentMessage<ChatResponse>>> + Unpin),
    ) -> Result<()> {
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    return Ok(());
                }
                maybe_message = stream.next() => {
                    match maybe_message {
                        Some(Ok(message)) => self.handle_chat_response(message)?,
                        Some(Err(err)) => {
                            return Err(err);
                        }
                        None => return Ok(()),
                    }
                }
            }
        }
    }

    async fn handle_dump(&mut self) -> Result<()> {
        if let Some(conversation_id) = self.state.conversation_id.clone() {
            let conversation = self.api.conversation(&conversation_id).await?;
            if let Some(conversation) = conversation {
                let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
                let path = self
                    .state
                    .current_title
                    .as_ref()
                    .map_or(format!("{timestamp}"), |title| {
                        format!("{timestamp}-{title}")
                    });

                let path = format!("{path}.json");

                let content = serde_json::to_string_pretty(&conversation)?;
                tokio::fs::write(path.as_str(), content).await?;

                CONSOLE.writeln(
                    TitleFormat::success("dump")
                        .sub_title(format!("path: {path}"))
                        .format(),
                )?;
            } else {
                CONSOLE.writeln(
                    TitleFormat::failed("dump")
                        .error("conversation not found")
                        .sub_title(format!("conversation_id: {conversation_id}"))
                        .format(),
                )?;
            }
        }
        Ok(())
    }

    fn handle_chat_response(&mut self, message: AgentMessage<ChatResponse>) -> Result<()> {
        match message.message {
            ChatResponse::Text(text) => {
                if message.agent.as_str() == "developer" {
                    CONSOLE.write(&text)?;
                }
            }
            ChatResponse::ToolCallStart(_) => {
                CONSOLE.newline()?;
                CONSOLE.newline()?;
            }
            ChatResponse::ToolCallEnd(tool_result) => {
                if !self.cli.verbose {
                    return Ok(());
                }

                let tool_name = tool_result.name.as_str();

                CONSOLE.writeln(format!("{}", tool_result.content.dimmed()))?;

                if tool_result.is_error {
                    CONSOLE.writeln(
                        TitleFormat::failed(tool_name)
                            .sub_title(self.state.usage.to_string())
                            .format(),
                    )?;
                } else {
                    CONSOLE.writeln(
                        TitleFormat::success(tool_name)
                            .sub_title(self.state.usage.to_string())
                            .format(),
                    )?;
                }
            }
            ChatResponse::Custom(event) => {
                if event.name == "title" {
                    self.state.current_title = Some(event.value);
                }
            }
            ChatResponse::Usage(u) => {
                self.state.usage = u;
            }
        }
        Ok(())
    }
}

const IMAGE_TYPES: &[&str] = &["jpg", "jpeg", "png", "gif", "webp"];

pub async fn prepare_attachments(paths: Vec<PathBuf>) -> Vec<Attachment> {
    futures::future::join_all(
        paths
            .into_iter()
            .filter(|v| v.extension().is_some())
            .filter(|v| IMAGE_TYPES.contains(&v.extension().unwrap().to_string_lossy().as_ref()))
            .map(|v| {
                let ext = v.extension().unwrap().to_string_lossy().to_string();
                tokio::fs::read(v).map_ok(|v| {
                    format!(
                        "data:image/{};base64,{}",
                        ext.strip_prefix('.').map(String::from).unwrap_or(ext),
                        base64::engine::general_purpose::STANDARD.encode(v)
                    )
                })
            }),
    )
    .await
    .into_iter()
    .filter_map(|v| v.ok())
    .map(|v| Attachment { data: v })
    .collect::<Vec<_>>()
}
