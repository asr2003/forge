use std::path::{Path, PathBuf};

use forge_walker::Walker;
use reedline::{Completer, Suggestion};
use tracing::info;

use crate::completer::search_term::SearchTerm;
use crate::completer::CommandCompleter;

#[derive(Clone)]
pub struct InputCompleter {
    walker: Walker,
}

impl InputCompleter {
    pub fn new(cwd: PathBuf) -> Self {
        let walker = Walker::max_all().cwd(cwd).skip_binary(true);
        Self { walker }
    }

    /// Check if path exists and is of supported type (directory or image)
    fn is_valid_path(&self, path: &Path) -> bool {
        if !path.exists() {
            return false;
        }

        // Always allow directories
        if path.is_dir() {
            return true;
        }

        // For files, only allow images
        if let Some(ext) = path.extension() {
            matches!(
                ext.to_str().unwrap_or("").to_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "gif" | "webp"
            )
        } else {
            false
        }
    }

    /// Get suggestions for file paths after /attach command
    fn get_attach_suggestions(&self, input: &str, span: reedline::Span) -> Vec<Suggestion> {
        let input_path = Path::new(input);
        let search_dir = if input_path.is_absolute() {
            if let Some(parent) = input_path.parent() {
                parent.to_path_buf()
            } else {
                PathBuf::from("/")
            }
        } else {
            if let Some(parent) = input_path.parent() {
                self.walker.get_cwd().join(parent)
            } else {
                self.walker.get_cwd()
            }
        };

        if !search_dir.exists() || !search_dir.is_dir() {
            return vec![];
        }

        let file_name = input_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        std::fs::read_dir(search_dir)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                let name = path.file_name()?.to_str()?.to_lowercase();

                if !name.starts_with(&file_name) {
                    return None;
                }

                if !self.is_valid_path(&path) {
                    return None;
                }

                let display = if path.is_dir() {
                    format!("{}/", entry.file_name().to_str()?)
                } else {
                    entry.file_name().to_str()?.to_string()
                };

                Some(Suggestion {
                    value: display.clone(),
                    description: None,
                    style: None,
                    extra: None,
                    span,
                    append_whitespace: true,
                })
            })
            .collect()
    }
}

impl Completer for InputCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        info!("Completing line: '{}' pos: {}", line, pos);

        // Handle /attach command completion
        if let Some(cmd) = line[..pos].strip_prefix("/attach ") {
            // Get the current word being completed
            let words: Vec<&str> = cmd.split_whitespace().collect();
            if let Some(last_word) = words.last() {
                return self.get_attach_suggestions(
                    last_word,
                    reedline::Span::new(pos - last_word.len(), pos),
                );
            }
            return self.get_attach_suggestions("", reedline::Span::new(pos, pos));
        }

        if line.starts_with("/") {
            // if the line starts with '/' it's probably a command, so we delegate to the
            // command completer.
            let result = CommandCompleter.complete(line, pos);
            if !result.is_empty() {
                return result;
            }
        }

        if let Some(query) = SearchTerm::new(line, pos).process() {
            info!("Search term: {:?}", query);

            let files = self.walker.get_blocking().unwrap_or_default();
            files
                .into_iter()
                .filter(|file| !file.is_dir())
                .filter_map(|file| {
                    if let Some(file_name) = file.file_name.as_ref() {
                        let file_name_lower = file_name.to_lowercase();
                        let query_lower = query.term.to_lowercase();
                        if file_name_lower.contains(&query_lower) {
                            Some(Suggestion {
                                value: file.path,
                                description: None,
                                style: None,
                                extra: None,
                                span: query.span,
                                append_whitespace: true,
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            vec![]
        }
    }
}
