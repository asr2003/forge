use std::collections::HashSet;
use std::path::Path;

use forge_tool_macros::Description as DescriptionDerive;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{Description, ToolCallService};

#[derive(Deserialize, JsonSchema)]
pub struct FSSearchInput {
    /// The path of the directory to search in (relative to the current working
    /// directory). This directory will be recursively searched.
    pub path: String,
    /// The regular expression pattern to search for. Uses Rust regex syntax.
    pub regex: String,
    /// Glob pattern to filter files (e.g., '*.ts' for TypeScript files). If not
    /// provided, it will search all files (*).
    pub file_pattern: Option<String>,
}

/// Request to perform a regex search across files in a specified directory,
/// providing context-rich results. This tool searches for patterns or specific
/// content across multiple files, displaying each match with encapsulating
/// context.
#[derive(DescriptionDerive)]
pub struct FSSearch;

#[async_trait::async_trait]
impl ToolCallService for FSSearch {
    type Input = FSSearchInput;
    type Output = Vec<String>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output, String> {
        use regex::Regex;
        use walkdir::WalkDir;

        let dir = Path::new(&input.path);
        if !dir.exists() {
            return Err("Directory does not exist".to_string());
        }

        // Create case-insensitive regex pattern
        let pattern = if input.regex.is_empty() {
            ".*".to_string()
        } else {
            format!("(?i){}", regex::escape(&input.regex)) // Add back regex::escape for literal matches
        };
        let regex = Regex::new(&pattern).map_err(|e| e.to_string())?;

        let mut matches = Vec::new();
        let mut seen_paths = HashSet::new();
        let walker = WalkDir::new(dir)
            .follow_links(false)
            .same_file_system(true)
            .into_iter();

        let entries = if let Some(ref pattern) = input.file_pattern {
            let glob = glob::Pattern::new(pattern).map_err(|e| e.to_string())?;
            walker
                .filter_entry(move |e| {
                    if !e.file_type().is_file() {
                        return true; // Keep traversing directories
                    }
                    e.file_name()
                        .to_str()
                        .map(|name| glob.matches(name))
                        .unwrap_or(false)
                })
                .filter_map(Result::ok)
                .collect::<Vec<_>>()
        } else {
            walker.filter_map(Result::ok).collect::<Vec<_>>()
        };

        for entry in entries {
            let path = entry.path().to_string_lossy();

            let name = entry.file_name().to_string_lossy();
            let is_file = entry.file_type().is_file();
            // let is_dir = entry.file_type().is_dir();

            // For empty pattern, only match files
            if input.regex.is_empty() {
                if is_file && seen_paths.insert(path.to_string()) {
                    matches.push(format!("File: {}\nLines 1-1:\n{}", path, path));
                }
                continue;
            }

            // Check filename and directory name for match
            if regex.is_match(&name) {
                if seen_paths.insert(path.to_string()) {
                    matches.push(format!("File: {}\nLines 1-1:\n{}", path, name));
                }
                if !is_file {
                    continue;
                }
            }

            // Skip content check for directories
            if !is_file {
                continue;
            }

            // Skip content check if already matched by name
            if seen_paths.contains(&path.to_string()) {
                continue;
            }

            // Check file content
            let content = match tokio::fs::read_to_string(entry.path()).await {
                Ok(content) => content,
                Err(_) => continue,
            };

            let lines: Vec<&str> = content.lines().collect();
            let mut content_matches = Vec::new();

            for (line_num, line) in lines.iter().enumerate() {
                if regex.is_match(line) {
                    // Get context (3 lines before and after)
                    let start = line_num.saturating_sub(3);
                    let end = (line_num + 4).min(lines.len());
                    let context = lines[start..end].join("\n");

                    content_matches.push(format!(
                        "File: {}\nLines {}-{}:\n{}\n",
                        path,
                        start + 1,
                        end,
                        context
                    ));
                }
            }

            if !content_matches.is_empty() {
                matches.extend(content_matches);
                seen_paths.insert(path.to_string());
            }
        }

        Ok(matches)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fs::tests::{File, FixtureBuilder};

    #[tokio::test]
    async fn test_fs_search_content() {
        let setup = FixtureBuilder::default()
            .files(vec![
                File::new("test.txt", "Hello test world"),
                File::new("other.txt", "No match here"),
                File::new("test2.txt", "Another test case"),
            ])
            .build()
            .await;

        let result = setup
            .run(
                FSSearch,
                FSSearchInput {
                    path: setup.path(),
                    regex: "test".to_string(),
                    file_pattern: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|p| p.contains("test.txt")));
        assert!(result.iter().any(|p| p.contains("test2.txt")));
    }

    #[tokio::test]
    async fn test_fs_search_with_pattern() {
        let setup = FixtureBuilder::default()
            .files(vec![
                File::new("test1.txt", "Hello test world"),
                File::new("test2.rs", "fn test() {}"),
            ])
            .build()
            .await;

        let result = setup
            .run(
                FSSearch,
                FSSearchInput {
                    path: setup.path(),
                    regex: "test".to_string(),
                    file_pattern: Some("*.rs".to_string()),
                },
            )
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert!(result.iter().any(|p| p.contains("test2.rs")));
    }

    #[tokio::test]
    async fn test_fs_search_with_context() {
        let content = "line 1\nline 2\ntest line\nline 4\nline 5";
        let setup = FixtureBuilder::default()
            .files(vec![File::new("test.txt", content)])
            .build()
            .await;
        let result = setup
            .run(
                FSSearch,
                FSSearchInput {
                    path: setup.path(),
                    regex: "test".to_string(),
                    file_pattern: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        let output = &result[0];
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 3);

        let output_path = lines[0].split(' ').last().unwrap();
        let output = std::fs::read_to_string(output_path).unwrap();

        assert!(output.contains("line 1"));
        assert!(output.contains("line 2"));
        assert!(output.contains("test line"));
        assert!(output.contains("line 4"));
        assert!(output.contains("line 5"));
    }

    #[tokio::test]
    async fn test_fs_search_recursive() {
        let setup = FixtureBuilder::default()
            .files(vec![
                File::new("test1.txt", ""),
                File::new("subdir/test2.txt", ""),
                File::new("subdir/other.txt", ""),
            ])
            .dirs(vec![String::from("subdir")])
            .build()
            .await;

        let result = setup
            .run(
                FSSearch,
                FSSearchInput {
                    path: setup.path(),
                    regex: "test".to_string(),
                    file_pattern: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|p| p.ends_with("test1.txt")));
        assert!(result.iter().any(|p| p.ends_with("test2.txt")));
    }

    #[tokio::test]
    async fn test_fs_search_case_insensitive() {
        let setup = FixtureBuilder::default()
            .files(vec![File::new("TEST.txt", ""), File::new("TeSt2.txt", "")])
            .build()
            .await;
        let result = setup
            .run(
                FSSearch,
                FSSearchInput {
                    path: setup.path(),
                    regex: "test".to_string(),
                    file_pattern: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|p| p.ends_with("TEST.txt")));
        assert!(result.iter().any(|p| p.ends_with("TeSt2.txt")));
    }

    #[tokio::test]
    async fn test_fs_search_empty_pattern() {
        let setup = FixtureBuilder::default()
            .files(vec![File::new("test.txt", "")])
            .build()
            .await;

        let result = setup
            .run(
                FSSearch,
                FSSearchInput {
                    path: setup.path(),
                    regex: "".to_string(),
                    file_pattern: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert!(result.iter().any(|p| p.ends_with("test.txt")));
    }

    #[tokio::test]
    async fn test_fs_search_nonexistent_directory() {
        let setup = FixtureBuilder::default().build().await;
        let result = setup
            .run(
                FSSearch,
                FSSearchInput {
                    path: setup.join("nonexistent"),
                    regex: "test".to_string(),
                    file_pattern: None,
                },
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fs_search_directory_names() {
        let setup = FixtureBuilder::default()
            .dirs(vec![
                String::from("test_dir"),
                String::from("other_dir"),
                String::from("test_dir/nested"),
            ])
            .build()
            .await;
        let result = setup
            .run(
                FSSearch,
                FSSearchInput {
                    path: setup.path(),
                    regex: "test".to_string(),
                    file_pattern: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert!(result.iter().any(|p| p.ends_with("test_dir")));
    }
}
