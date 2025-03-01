use std::path::{Path, PathBuf};

use anyhow::bail;
use forge_display::DiffFormat;
use forge_domain::{ExecutableTool, NamedTool, ToolDescription, ToolName};
use schemars::JsonSchema;
use serde::Deserialize;
use thiserror::Error;
use tokio::fs;

use super::marker::{DIVIDER, REPLACE, SEARCH};
use super::parse::{self, PatchBlock};
use crate::tools::syn;
use crate::tools::utils::assert_absolute_path;

#[derive(Debug, Error)]
enum Error {
    #[error("File not found at path: {0}")]
    FileNotFound(PathBuf),
    #[error("File operation failed: {0}")]
    FileOperation(#[from] std::io::Error),
}

/// Input parameters for the fs_replace tool.
#[derive(Deserialize, JsonSchema)]
pub struct ApplyPatchInput {
    /// File path (absolute path required)
    pub path: String,
    /// Multiple SEARCH/REPLACE blocks separated by newlines, defining changes
    /// to make to the file.
    pub diff: String,
}

pub struct ApplyPatch;

impl NamedTool for ApplyPatch {
    fn tool_name() -> ToolName {
        ToolName::new("tool_forge_fs_patch")
    }
}

impl ToolDescription for ApplyPatch {
    fn description(&self) -> String {
        format!(
            r#"Replace sections in a file using multiple SEARCH/REPLACE blocks. Example:
{SEARCH}
[exact content to find]
{DIVIDER}
[new content to replace with]
{REPLACE}

Rules:
1. SEARCH must exactly match whitespace, indentation & line endings
2. Each block replaces first match only
3. Keep blocks minimal - include only changing lines plus needed context
4. Provide complete lines only - no truncation
5. Use multiple blocks for multiple changes in the same file
6. For moves: use 2 blocks (delete block + insert block)
7. For deletes: use empty REPLACE section

Example with multiple blocks:
{SEARCH}
def old_function(x):
    return x + 1
{DIVIDER}
def new_function(x, y=0):
    return x + y
{REPLACE}
{SEARCH}
# Old comment
{DIVIDER}
# Updated documentation - now supports multiple parameters
{REPLACE}
        "#
        )
        .trim()
        .to_string()
    }
}

/// Start and end indices align with UTF-8 character boundaries
fn get_utf8_safe_indices(s: &str, byte_start: usize, byte_end: usize) -> Option<(usize, usize)> {
    let start = s
        .char_indices()
        .find(|&(i, _)| i == byte_start)
        .map(|(i, _)| i)?;
    let end = s
        .char_indices()
        .find(|&(i, _)| i == byte_end)
        .map(|(i, _)| i)?;
    Some((start, end))
}

/// Replaces a substring only if UTF-8 boundaries are valid
fn safe_replace_range(content: &mut String, start: usize, end: usize, replacement: &str) {
    if let Some((safe_start, safe_end)) = get_utf8_safe_indices(content, start, end) {
        content.replace_range(safe_start..safe_end, replacement);
    }
}

/// Apply changes to file content based on search/replace blocks.
/// Changes are only written to disk if all replacements are successful.
async fn apply_patches(content: String, blocks: Vec<PatchBlock>) -> Result<String, Error> {
    let mut result = content;

    // Apply each block sequentially
    for block in blocks {
        // For empty search string, append the replacement text at the end of file.
        if block.search.is_empty() {
            result.push_str(&block.replace);
            continue;
        }

        // For exact matching, first try to find the exact string
        if let Some(start_idx) = result.find(&block.search) {
            let end_idx = start_idx + block.search.len();
            safe_replace_range(&mut result, start_idx, end_idx, &block.replace);
        }
    }
    Ok(result)
}

#[async_trait::async_trait]
impl ExecutableTool for ApplyPatch {
    type Input = ApplyPatchInput;

    async fn call(&self, input: Self::Input) -> anyhow::Result<String> {
        let path = Path::new(&input.path);
        assert_absolute_path(path)?;

        if !path.exists() {
            bail!(Error::FileNotFound(path.to_path_buf()));
        }

        let blocks = parse::parse_blocks(&input.diff)?;

        // Read the content of the file before applying the patch
        let old_content = fs::read_to_string(&input.path)
            .await
            .map_err(Error::FileOperation)?;

        let result = async {
            let modified = apply_patches(old_content.clone(), blocks).await?;
            fs::write(&input.path, &modified)
                .await
                .map_err(Error::FileOperation)?;

            let syntax_warning = syn::validate(&input.path, &modified);

            // Handle syntax warning and build output
            let output = if let Some(warning) = syntax_warning {
                format!(
                    "<file_content\n  path=\"{}\"\n  syntax_checker_warning=\"{}\">\n{}</file_content>\n",
                    input.path,
                    warning,
                    modified
                )
            } else {
                format!(
                    "<file_content path=\"{}\">\n{}\n</file_content>\n",
                    input.path,
                    modified.trim_end()
                )
            };
            anyhow::Ok(output)
        }
         .await?;

        // record the content of the file after applying the patch
        let new_content = fs::read_to_string(path)
            .await
            .map_err(Error::FileOperation)?;
        // Generate diff between old and new content
        let diff = DiffFormat::format(path.to_path_buf(), &old_content, &new_content);
        println!("{}", diff);

        Ok(result)
    }
}

#[cfg(test)]
mod test {
    use std::io::{Error as IoError, ErrorKind as IoErrorKind};

    use super::*;
    use crate::tools::utils::TempDir;

    async fn write_test_file(path: impl AsRef<Path>, content: &str) -> Result<(), Error> {
        fs::write(&path, content)
            .await
            .map_err(Error::FileOperation)
    }

    #[test]
    fn test_error_messages() {
        // Test file not found error
        let err = Error::FileNotFound(PathBuf::from("nonexistent.txt"));
        insta::assert_snapshot!(err.to_string());

        // Test file operation error
        let io_err = Error::FileOperation(IoError::new(
            IoErrorKind::NotFound,
            "No such file or directory (os error 2)",
        ));
        insta::assert_snapshot!(io_err.to_string());
    }

    #[tokio::test]
    async fn test_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("nonexistent.txt");

        let fs_replace = ApplyPatch;
        let result = fs_replace
            .call(ApplyPatchInput {
                path: nonexistent.to_string_lossy().to_string(),
                diff: format!("{SEARCH}\nHello\n{DIVIDER}\nWorld\n{REPLACE}\n"),
            })
            .await;

        assert!(result.unwrap_err().to_string().contains("File not found"));
    }

    #[tokio::test]
    async fn test_whitespace_preservation() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let content = "    Hello World    \n  Test Line  \n   Goodbye World   \n";

        write_test_file(&file_path, content).await.unwrap();

        let fs_replace = ApplyPatch;
        let result = fs_replace
            .call(ApplyPatchInput {
                path: file_path.to_string_lossy().to_string(),
                diff: format!(
                    "{SEARCH}\n    Hello World    \n{DIVIDER}\n    Hi World    \n{REPLACE}\n"
                )
                .to_string(),
            })
            .await
            .unwrap();

        insta::assert_snapshot!(TempDir::normalize(&result));

        // Also snapshot the final file content to verify whitespace preservation
        let final_content = fs::read_to_string(&file_path).await.unwrap();
        insta::assert_snapshot!(final_content);
    }

    #[tokio::test]
    async fn test_empty_search_new_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        write_test_file(&file_path, "").await.unwrap();

        let fs_replace = ApplyPatch;
        let result = fs_replace
            .call(ApplyPatchInput {
                path: file_path.to_string_lossy().to_string(),
                diff: format!("{SEARCH}\n{DIVIDER}\nNew content\n{REPLACE}\n").to_string(),
            })
            .await
            .unwrap();

        insta::assert_snapshot!(TempDir::normalize(&result));

        // Also snapshot the final file content
        let final_content = fs::read_to_string(&file_path).await.unwrap();
        insta::assert_snapshot!(final_content);
    }

    #[tokio::test]
    async fn test_multiple_blocks() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let content = "    First Line    \n  Middle Line  \n    Last Line    \n";

        write_test_file(&file_path, content).await.unwrap();

        let fs_replace = ApplyPatch;
        let diff = format!("{SEARCH}\n    First Line    \n{DIVIDER}\n    New First    \n{REPLACE}\n{SEARCH}\n    Last Line    \n{DIVIDER}\n    New Last    \n{REPLACE}\n").to_string();

        let result = fs_replace
            .call(ApplyPatchInput { path: file_path.to_string_lossy().to_string(), diff })
            .await
            .unwrap();

        insta::assert_snapshot!(TempDir::normalize(&result));

        // Also snapshot the final file content to verify both replacements
        let final_content = fs::read_to_string(&file_path).await.unwrap();
        insta::assert_snapshot!(final_content);
    }

    #[tokio::test]
    async fn test_empty_block() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let content = "    First Line    \n  Middle Line  \n    Last Line    \n";

        write_test_file(&file_path, content).await.unwrap();

        let fs_replace = ApplyPatch;
        let diff = format!("{SEARCH}\n  Middle Line  \n{DIVIDER}\n{REPLACE}\n");
        let result = fs_replace
            .call(ApplyPatchInput { path: file_path.to_string_lossy().to_string(), diff })
            .await
            .unwrap();

        insta::assert_snapshot!(TempDir::normalize(&result));

        // Also snapshot the final file content to verify the line was removed
        let final_content = fs::read_to_string(&file_path).await.unwrap();
        insta::assert_snapshot!(final_content);
    }

    #[tokio::test]
    async fn test_complex_newline_preservation() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // Test file with various newline patterns
        let content = "\n\n// Header comment\n\n\nfunction test() {\n    // Inside comment\n\n    let x = 1;\n\n\n    console.log(x);\n}\n\n// Footer comment\n\n\n";
        write_test_file(&file_path, content).await.unwrap();

        let fs_replace = ApplyPatch;

        // Test 1: Replace content while preserving surrounding newlines
        let result = fs_replace
            .call(ApplyPatchInput {
                path: file_path.to_string_lossy().to_string(),
                diff: format!("{SEARCH}\n    let x = 1;\n\n\n    console.log(x);\n{DIVIDER}\n    let y = 2;\n\n\n    console.log(y);\n{REPLACE}\n").to_string(),
            })
            .await
            .unwrap();

        insta::assert_snapshot!(TempDir::normalize(&result));
        let content1 = fs::read_to_string(&file_path).await.unwrap();
        insta::assert_snapshot!(content1);

        // Test 2: Replace block with different newline pattern
        let result = fs_replace
            .call(ApplyPatchInput {
                path: file_path.to_string_lossy().to_string(),
                diff: format!(
                    "{SEARCH}\n\n// Footer comment\n\n\n{DIVIDER}\n\n\n\n// Updated footer\n\n{REPLACE}\n"
                )
                .to_string(),
            })
            .await
            .unwrap();

        insta::assert_snapshot!(TempDir::normalize(&result));
        let content2 = fs::read_to_string(&file_path).await.unwrap();
        insta::assert_snapshot!(content2);

        // Test 3: Replace with empty lines preservation
        let result = fs_replace
            .call(ApplyPatchInput {
                path: file_path.to_string_lossy().to_string(),
                diff: format!(
                    "{SEARCH}\n\n\n// Header comment\n\n\n{DIVIDER}\n\n\n\n// New header\n\n\n\n{REPLACE}\n"
                )
                .to_string(),
            })
            .await
            .unwrap();

        insta::assert_snapshot!(TempDir::normalize(&result));
        let content3 = fs::read_to_string(&file_path).await.unwrap();
        insta::assert_snapshot!(content3);
    }

    #[tokio::test]
    async fn test_fuzzy_search_replace() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // Test file with typos and variations
        let content = r#"function calculateTotal(items) {
  let total = 0;
  for (const itm of items) {
    total += itm.price;
  }
  return total;
}
"#;
        write_test_file(&file_path, content).await.unwrap();

        let fs_replace = ApplyPatch;
        // Search with different casing, spacing, and variable names
        let result = fs_replace
            .call(ApplyPatchInput {
                path: file_path.to_string_lossy().to_string(),
                diff: format!("{SEARCH}\n  for (const itm of items) {{\n    total += itm.price;\n{DIVIDER}\n  for (const item of items) {{\n    total += item.price * item.quantity;\n{REPLACE}\n").to_string(),
            })
            .await
            .unwrap();

        insta::assert_snapshot!(TempDir::normalize(&result));
        let content1 = fs::read_to_string(&file_path).await.unwrap();
        insta::assert_snapshot!(content1);

        // Test fuzzy matching with more variations
        let result = fs_replace
            .call(ApplyPatchInput {
                path: file_path.to_string_lossy().to_string(),
                diff: format!("{SEARCH}\nfunction calculateTotal(items) {{\n  let total = 0;\n{DIVIDER}\nfunction computeTotal(items, tax = 0) {{\n  let total = 0.0;\n{REPLACE}\n").to_string(),
            })
            .await
            .unwrap();

        insta::assert_snapshot!(TempDir::normalize(&result));
        let content2 = fs::read_to_string(&file_path).await.unwrap();
        insta::assert_snapshot!(content2);
    }

    #[tokio::test]
    async fn test_fuzzy_search_advanced() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // Test file with more complex variations
        let content = r#"class UserManager {
  async getUserById(userId) {
    const user = await db.findOne({ id: userId });
    if (!user) throw new Error('User not found');
    return user;
  }
}
"#;
        write_test_file(&file_path, content).await.unwrap();

        let fs_replace = ApplyPatch;
        // Search with structural similarities but different variable names and spacing
        let result = fs_replace
            .call(ApplyPatchInput {
                path: file_path.to_string_lossy().to_string(),
                diff: format!("{SEARCH}\n  async getUserById(userId) {{\n    const user = await db.findOne({{ id: userId }});\n{DIVIDER}\n  async findUser(id, options = {{}}) {{\n    const user = await this.db.findOne({{ userId: id, ...options }});\n{REPLACE}\n").to_string(),
            })
            .await
            .unwrap();

        insta::assert_snapshot!(TempDir::normalize(&result));
        let content1 = fs::read_to_string(&file_path).await.unwrap();
        insta::assert_snapshot!(content1);

        // Test fuzzy matching with error handling changes
        let result = fs_replace
            .call(ApplyPatchInput {
                path: file_path.to_string_lossy().to_string(),
                diff: format!("{SEARCH}\n    if (!user) throw new Error('User not found');\n    return user;\n{DIVIDER}\n    if (!user) {{\n      throw new UserNotFoundError(id);\n    }}\n    return this.sanitizeUser(user);\n{REPLACE}\n").to_string(),
            })
            .await
            .unwrap();

        insta::assert_snapshot!(TempDir::normalize(&result));
        let content2 = fs::read_to_string(&file_path).await.unwrap();
        insta::assert_snapshot!(content2);
    }

    #[tokio::test]
    async fn test_invalid_rust_replace() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        let content = "fn main() { let x = 42; }";

        write_test_file(&file_path, content).await.unwrap();

        let fs_replace = ApplyPatch;
        let result = fs_replace
            .call(ApplyPatchInput {
                path: file_path.to_string_lossy().to_string(),
                diff: format!(
                    "{SEARCH}\nfn main() {{ let x = 42; }}\n{DIVIDER}\nfn main() {{ let x = \n{REPLACE}\n"
                )
                .to_string(),
            })
            .await
            .unwrap();

        insta::assert_snapshot!(TempDir::normalize(&result));
        let content = fs::read_to_string(&file_path).await.unwrap();
        insta::assert_snapshot!(content);
    }

    #[tokio::test]
    async fn test_valid_rust_replace() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        let content = "fn main() { let x = 42; }";

        write_test_file(&file_path, content).await.unwrap();

        let fs_replace = ApplyPatch;
        let result = fs_replace
            .call(ApplyPatchInput {
                path: file_path.to_string_lossy().to_string(),
                diff: format!("{SEARCH}\nfn main() {{ let x = 42; }}\n{DIVIDER}\nfn main() {{ let x = 42; let y = x * 2; }}\n{REPLACE}\n").to_string(),
            })
            .await
            .unwrap();

        insta::assert_snapshot!(TempDir::normalize(&result));
        let content = fs::read_to_string(&file_path).await.unwrap();
        insta::assert_snapshot!(content);
    }

    #[tokio::test]
    async fn test_patch_relative_path() {
        let fs_replace = ApplyPatch;
        let result = fs_replace
            .call(ApplyPatchInput {
                path: "relative/path.txt".to_string(),
                diff: format!("{SEARCH}\ntest\n{DIVIDER}\nreplacement\n{REPLACE}\n"),
            })
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Path must be absolute"));
    }
}
