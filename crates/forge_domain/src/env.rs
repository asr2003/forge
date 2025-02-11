use std::path::PathBuf;

use derive_setters::Setters;
use serde::{Deserialize, Serialize};

use crate::ModelId;

#[derive(Debug, Setters, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[setters(strip_option)]
/// Represents the environment in which the application is running.
pub struct Environment {
    /// The operating system of the environment.
    pub os: String,
    /// The current working directory.
    pub cwd: PathBuf,
    /// The home directory.
    pub home: Option<PathBuf>,
    /// The shell being used.
    pub shell: String,
    /// The Forge API key.
    pub api_key: String,
    /// The large model ID.
    pub large_model_id: ModelId,
    /// The small model ID.
    pub small_model_id: ModelId,

    /// The base path relative to which everything else stored.
    pub base_path: PathBuf,
}

impl Environment {
    pub fn db_path(&self) -> PathBuf {
        self.base_path.clone()
    }

    pub fn log_path(&self) -> PathBuf {
        self.base_path.join("logs")
    }

    pub fn history_path(&self) -> PathBuf {
        self.base_path.join(".forge_history")
    }
}
