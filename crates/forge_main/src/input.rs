use std::path::PathBuf;

use async_trait::async_trait;
use forge_domain::{Command, Usage, UserInput};
use tokio::fs;

use crate::console::CONSOLE;
use crate::prompting_engine::{CustomPrompt, ReadResult, ReedLineEngine};
use crate::StatusDisplay;

/// Console implementation for handling user input via command line.
#[derive(Debug, Default)]
pub struct Console;

#[async_trait]
impl UserInput for Console {
    async fn upload<P: Into<PathBuf> + Send>(&self, path: P) -> anyhow::Result<Command> {
        let path = path.into();
        let content = fs::read_to_string(&path).await?.trim().to_string();

        CONSOLE.writeln(content.clone())?;
        Ok(Command::Message(content))
    }

    async fn prompt(
        &self,
        help_text: Option<&str>,
        _initial_text: Option<&str>,
    ) -> anyhow::Result<Command> {
        CONSOLE.writeln("")?;
        loop {
            // let help = help_text.map(|a| a.to_string()).unwrap_or(format!(
            //     "Available commands: {}",
            //     Command::available_commands().join(", ")
            // ));

            // let mut text = inquire::Text::new("")
            //     .with_help_message(&help)
            //     .with_autocomplete(CommandCompleter::new());

            // if let Some(initial_text) = initial_text {
            //     text = text.with_initial_value(initial_text);
            // }
            let result = if let Some(help_text) = help_text {
                ReedLineEngine::start()
                    .with_prompt(Box::new(CustomPrompt::default().with_title(help_text)))
                    .prompt()?
            } else {
                ReedLineEngine::start().prompt()?
            };
            match result {
                ReadResult::Continue => continue,
                ReadResult::Exit => return Ok(Command::Exit),
                ReadResult::Success(text) => match Command::parse(&text) {
                    Ok(input) => return Ok(input),
                    Err(e) => {
                        CONSOLE.writeln(
                            StatusDisplay::failed(e.to_string(), Usage::default()).format(),
                        )?;
                    }
                },
            }
        }
    }
}
