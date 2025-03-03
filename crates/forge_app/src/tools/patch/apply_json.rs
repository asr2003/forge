use std::path::Path;

// No longer using dissimilar for fuzzy matching
use forge_domain::{ExecutableTool, NamedTool, ToolDescription, ToolName};
use forge_tool_macros::ToolDescription;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::fs;

use crate::tools::syn;
use crate::tools::utils::assert_absolute_path;

// Removed fuzzy matching threshold as we only use exact matching now

/// A match found in the source text. Represents a range in the source text that
/// can be used for extraction or replacement operations. Stores the position
/// and length to allow efficient substring operations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct Range {
    /// Starting position of the match in source text
    start: usize,
    /// Length of the matched text
    length: usize,
}

impl Range {
    /// Create a new match from a start position and length
    fn new(start: usize, length: usize) -> Self {
        Self { start, length }
    }

    /// Get the end position (exclusive) of this match
    fn end(&self) -> usize {
        self.start + self.length
    }

    /// Try to find an exact match in the source text
    fn find_exact(source: &str, search: &str) -> Option<Self> {
        source
            .find(search)
            .map(|start| Self::new(start, search.len()))
    }

    // Fuzzy matching removed - we only use exact matching
}

impl From<Range> for std::ops::Range<usize> {
    fn from(m: Range) -> Self {
        m.start..m.end()
    }
}

// MatchSequence struct and implementation removed - we only use exact matching

#[derive(Debug, Error)]
enum Error {
    #[error("Failed to read/write file: {0}")]
    FileOperation(#[from] std::io::Error),
    #[error("Could not find match for search text: {0}")]
    NoMatch(String),
    #[error("Could not find swap target text: {0}")]
    NoSwapTarget(String),
}

fn apply_replacement(
    source: String,
    search: &str,
    operation: &Operation,
    content: &str,
) -> Result<String, Error> {
    // Handle empty search string - only certain operations make sense here
    if search.is_empty() {
        return match operation {
            // Append to the end of the file
            Operation::Append => Ok(format!("{}{}", source, content)),
            // Prepend to the beginning of the file
            Operation::Prepend => Ok(format!("{}{}", content, source)),
            // Replace is equivalent to completely replacing the file
            Operation::Replace => Ok(content.to_string()),
            // Swap doesn't make sense with empty search - keep source unchanged
            Operation::Swap => Ok(source),
        };
    }

    // Find the exact match to operate on
    let patch =
        Range::find_exact(&source, search).ok_or_else(|| Error::NoMatch(search.to_string()))?;

    // Apply the operation based on its type
    match operation {
        // Prepend content before the matched text
        Operation::Prepend => Ok(format!(
            "{}{}{}",
            &source[..patch.start],
            content,
            &source[patch.start..]
        )),

        // Append content after the matched text
        Operation::Append => Ok(format!(
            "{}{}{}",
            &source[..patch.end()],
            content,
            &source[patch.end()..]
        )),

        // Replace matched text with new content
        Operation::Replace => Ok(format!(
            "{}{}{}",
            &source[..patch.start],
            content,
            &source[patch.end()..]
        )),

        // Swap with another text in the source
        Operation::Swap => {
            // Find the target text to swap with
            let target_patch = Range::find_exact(&source, content)
                .ok_or_else(|| Error::NoSwapTarget(content.to_string()))?;

            // Handle the case where patches overlap
            if (patch.start <= target_patch.start && patch.end() > target_patch.start)
                || (target_patch.start <= patch.start && target_patch.end() > patch.start)
            {
                // For overlapping ranges, we just do an ordinary replacement
                return Ok(format!(
                    "{}{}{}",
                    &source[..patch.start],
                    content,
                    &source[patch.end()..]
                ));
            }

            // We need to handle different ordering of patches
            if patch.start < target_patch.start {
                // Original text comes first
                Ok(format!(
                    "{}{}{}{}{}",
                    &source[..patch.start],
                    content,
                    &source[patch.end()..target_patch.start],
                    &source[patch.start..patch.end()],
                    &source[target_patch.end()..]
                ))
            } else {
                // Target text comes first
                Ok(format!(
                    "{}{}{}{}{}",
                    &source[..target_patch.start],
                    &source[patch.start..patch.end()],
                    &source[target_patch.end()..patch.start],
                    content,
                    &source[patch.end()..]
                ))
            }
        }
    }
}

/// Operation types that can be performed on matched text
#[derive(Deserialize, Serialize, JsonSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// Prepend content before the matched text
    Prepend,

    /// Append content after the matched text
    Append,

    /// Replace the matched text with new content
    Replace,

    /// Swap the matched text with another text (search for the second text and
    /// swap them)
    Swap,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ApplyPatchJsonInput {
    /// The path to the file to modify
    pub path: String,

    /// The text to search for in the source. If empty, operation applies to the
    /// end of the file.
    pub search: String,

    /// The operation to perform on the matched text
    pub operation: Operation,

    /// The content to use for the operation (replacement text, text to
    /// prepend/append, or target text for swap operations)
    pub content: String,
}

/// Performs a single text operation (prepend, append, replace, swap, delete) on
/// matched text in a file. The operation is applied to the first match found in
/// the text.
#[derive(ToolDescription)]
pub struct ApplyPatchJson;

impl NamedTool for ApplyPatchJson {
    fn tool_name() -> ToolName {
        ToolName::new("tool_forge_fs_patch")
    }
}

/// Format the modified content as XML with optional syntax warning
fn format_output(path: &str, content: &str, warning: Option<&str>) -> String {
    if let Some(w) = warning {
        format!(
            "<file_content\n  path=\"{}\"\n  syntax_checker_warning=\"{}\">\n{}</file_content>\n",
            path, w, content
        )
    } else {
        format!(
            "<file_content path=\"{}\">\n{}\n</file_content>\n",
            path,
            content.trim_end()
        )
    }
}

/// Process the file modification and return the formatted output
async fn process_file_modifications(
    path: &Path,
    search: &str,
    operation: &Operation,
    content: &str,
) -> Result<String, Error> {
    let file_content = fs::read_to_string(path).await?;
    let file_content = apply_replacement(file_content, search, operation, content)?;
    fs::write(path, &file_content).await?;

    let warning = syn::validate(path, &file_content).map(|e| e.to_string());
    Ok(format_output(
        path.to_string_lossy().as_ref(),
        &file_content,
        warning.as_deref(),
    ))
}

#[async_trait::async_trait]
impl ExecutableTool for ApplyPatchJson {
    type Input = ApplyPatchJsonInput;

    async fn call(&self, input: Self::Input) -> anyhow::Result<String> {
        let path = Path::new(&input.path);
        assert_absolute_path(path)?;

        Ok(
            process_file_modifications(path, &input.search, &input.operation, &input.content)
                .await?,
        )
    }
}

#[cfg(test)]
mod test {

    use super::*;

    // Enhanced test helper for running multiple operations
    #[derive(Debug)]
    struct PatchTest {
        initial: String,
        patches: Vec<Patch>,
    }

    // Single operation with its result
    #[derive(Debug)]
    struct Patch {
        operation: PatchOperation,
        result: Result<String, String>,
    }

    // Represents a single patch operation
    #[derive(Debug)]
    struct PatchOperation {
        search: String,
        operation: Operation,
        content: String,
    }

    // fmt::Display implementation removed in favor of using assert_debug_snapshot!

    impl PatchTest {
        fn new(initial: impl ToString) -> Self {
            PatchTest { initial: initial.to_string(), patches: Vec::new() }
        }

        /// Replace matched text with new content
        fn replace(mut self, search: impl ToString, content: impl ToString) -> Self {
            let operation = PatchOperation {
                search: search.to_string(),
                operation: Operation::Replace,
                content: content.to_string(),
            };
            self.patches.push(Patch {
                operation,
                result: Err("Not executed yet".to_string()), // Placeholder
            });
            self
        }

        /// Prepend content before matched text
        fn prepend(mut self, search: impl ToString, content: impl ToString) -> Self {
            let operation = PatchOperation {
                search: search.to_string(),
                operation: Operation::Prepend,
                content: content.to_string(),
            };
            self.patches.push(Patch {
                operation,
                result: Err("Not executed yet".to_string()), // Placeholder
            });
            self
        }

        /// Append content after matched text
        fn append(mut self, search: impl ToString, content: impl ToString) -> Self {
            let operation = PatchOperation {
                search: search.to_string(),
                operation: Operation::Append,
                content: content.to_string(),
            };
            self.patches.push(Patch {
                operation,
                result: Err("Not executed yet".to_string()), // Placeholder
            });
            self
        }

        /// Swap matched text with target text
        fn swap(mut self, search: impl ToString, target: impl ToString) -> Self {
            let operation = PatchOperation {
                search: search.to_string(),
                operation: Operation::Swap,
                content: target.to_string(),
            };
            self.patches.push(Patch {
                operation,
                result: Err("Not executed yet".to_string()), // Placeholder
            });
            self
        }

        /// Try to execute all operations and record their results
        fn execute_all(mut self) -> Self {
            let mut current_content = self.initial.clone();

            for op_result in &mut self.patches {
                // Apply the operation
                let result = match apply_replacement(
                    current_content.clone(),
                    &op_result.operation.search,
                    &op_result.operation.operation,
                    &op_result.operation.content,
                ) {
                    Ok(content) => {
                        // Update the current content for the next operation
                        current_content = content.clone();
                        Ok(content)
                    }
                    Err(err) => Err(err.to_string()),
                };

                // Update the result
                op_result.result = result;
            }

            self
        }
    }

    #[test]
    fn comprehensive_patch_tests() {
        // Create a comprehensive test that includes all the test cases
        let test = PatchTest::new("Hello World")
            // Basic Operations
            .replace("World", "Forge")
            .replace("", " bar")
            // Single Replacement Behavior
            .replace("foo", "baz")
            // Exact Matching Behavior
            .replace("Hello", "Hi")
            // Unicode and Special Characters
            .replace("Hello", "你好")
            .replace("World", "🌍")
            // Whitespace Handling
            .prepend("Hello", "    ")
            .append("World", "\n  New line")
            // Test different operation types
            .prepend("Hello", "Greetings, ")
            .append("World", "!")
            .swap("Hello", "World")
            // Empty search operations
            .prepend("", "Start: ")
            .append("", " End")
            .replace("", "Completely New Content")
            // Execute all operations and collect results
            .execute_all();

        // Snapshot the entire test result using Debug representation
        insta::assert_debug_snapshot!(test);
    }

    #[test]
    fn comprehensive_error_tests() {
        // Create a test specifically for error cases
        let test = PatchTest::new("foo bar baz")
            .replace("nonexistent", "replaced")
            .replace("foo-bar", "replaced")
            .replace("afoo", "replaced")
            .swap("foo", "nonexistent")
            .execute_all();

        // Snapshot the error test results using Debug representation
        insta::assert_debug_snapshot!(test);
    }

    // The previous individual tests are removed since they're now consolidated
}
