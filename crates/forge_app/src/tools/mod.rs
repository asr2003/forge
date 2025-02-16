mod fetch;
mod fs;
#[allow(unused)]
mod knowledge;
mod patch;
mod shell;
mod syn;
mod think;
mod utils;

use fetch::Fetch;
use forge_domain::{Environment, Tool};
use fs::*;
use patch::*;
use shell::Shell;
use think::Think;

pub fn tools(env: &Environment) -> Vec<Tool> {
    vec![
        // Approve.into(),
        FSRead.into(),
        FSWrite.into(),
        FSRemove.into(),
        FSList::default().into(),
        FSSearch.into(),
        FSFileInfo.into(),
        // TODO: once ApplyPatchJson is stable we can delete ApplyPatch
        ApplyPatch.into(),
        // ApplyPatchJson.into(),
        // Outline.into(),
        // SelectTool.into(),
        Shell::new(env.clone()).into(),
        Think::default().into(),
        Fetch::default().into(),
    ]
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// Create a default test environment
    fn test_env() -> Environment {
        Environment {
            os: std::env::consts::OS.to_string(),
            cwd: std::env::current_dir().unwrap_or_default(),
            home: Some("/".into()),
            shell: if cfg!(windows) {
                "cmd.exe".to_string()
            } else {
                "/bin/sh".to_string()
            },
            open_router_key: String::new(),
            base_path: PathBuf::new(),
            qdrant_key: None,
            qdrant_cluster: None,
        }
    }

    #[test]
    fn test_tool_description_length() {
        const MAX_DESCRIPTION_LENGTH: usize = 1024;

        println!("\nTool description lengths:");

        let mut any_exceeded = false;
        let env = test_env();
        for tool in tools(&env) {
            let desc_len = tool.definition.description.len();
            println!(
                "{:?}: {} chars {}",
                tool.definition.name,
                desc_len,
                if desc_len > MAX_DESCRIPTION_LENGTH {
                    "(!)"
                } else {
                    ""
                }
            );

            if desc_len > MAX_DESCRIPTION_LENGTH {
                any_exceeded = true;
            }
        }

        assert!(
            !any_exceeded,
            "One or more tools exceed the maximum description length of {}",
            MAX_DESCRIPTION_LENGTH
        );
    }
}
