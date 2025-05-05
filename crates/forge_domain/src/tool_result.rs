use derive_setters::Setters;
use gray_matter::engine::YAML;
use gray_matter::Matter;
use serde::{Deserialize, Serialize};

use crate::{ToolCallFull, ToolCallId, ToolName};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Setters)]
#[setters(strip_option, into)]
pub struct ToolResult {
    pub name: ToolName,
    pub call_id: Option<ToolCallId>,
    #[setters(skip)]
    pub content: String,
    #[setters(skip)]
    pub is_error: bool,
}

#[derive(Serialize, Deserialize)]
struct ToolResultFrontMatter {
    name: String,
    call_id: Option<String>,
    status: String,
}

impl ToolResult {
    pub fn new(name: ToolName) -> ToolResult {
        Self {
            name,
            call_id: None,
            content: String::default(),
            is_error: false,
        }
    }

    pub fn success(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self.is_error = false;
        self
    }

    pub fn failure(mut self, err: anyhow::Error) -> Self {
        let mut output = String::new();
        output.push_str("\nERROR:\n");

        for cause in err.chain() {
            output.push_str(&format!("Caused by: {cause}\n"));
        }

        self.content = output;
        self.is_error = true;
        self
    }

    /// Emit `ToolResult` as a Front Matter formatted string
    fn to_front_matter(&self) -> String {
        let front_matter = ToolResultFrontMatter {
            name: self.name.as_str().to_string(),
            call_id: self.call_id.as_ref().map(|id| id.as_str().to_string()),
            status: if self.is_error {
                "error".to_string()
            } else {
                "success".to_string()
            },
        };

        let mut yaml = String::new();
        yaml.push_str(&format!("name: {}\n", front_matter.name));

        if let Some(call_id) = &front_matter.call_id {
            yaml.push_str(&format!("call_id: {}\n", call_id));
        } else {
            yaml.push_str("call_id: null\n");
        }

        yaml.push_str(&format!("status: {}\n", front_matter.status));

        format!("---\n{}---\n{}", yaml, self.content)
    }

    pub fn from_frontmatter(input: &str) -> anyhow::Result<Self> {
        let parsed = Matter::<YAML>::new()
            .parse_with_struct::<ToolResultFrontMatter>(input)
            .ok_or_else(|| {
                anyhow::anyhow!("Failed to parse front matter into ToolResultFrontMatter")
            })?;
        let front = parsed.data;
        let content = parsed.content;

        let name = ToolName::new(&front.name);
        let call_id = front.call_id.map(|id| ToolCallId::new(&id));
        let is_error = front.status == "error";

        Ok(Self { name, call_id, content, is_error })
    }
}

impl From<ToolCallFull> for ToolResult {
    fn from(value: ToolCallFull) -> Self {
        Self {
            name: value.name,
            call_id: value.call_id,
            content: String::default(),
            is_error: false,
        }
    }
}

impl std::fmt::Display for ToolResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_front_matter())
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;
    use serde_json::json;

    use super::*;

    #[test]
    fn test_snapshot_minimal() {
        let result = ToolResult::new(ToolName::new("test_tool"));
        assert_snapshot!(result);
    }

    #[test]
    fn test_snapshot_full() {
        let result = ToolResult::new(ToolName::new("complex_tool"))
            .call_id(ToolCallId::new("123"))
            .failure(anyhow::anyhow!(
                json!({"key": "value", "number": 42}).to_string()
            ));
        assert_snapshot!(result);
    }

    #[test]
    fn test_snapshot_with_special_chars() {
        let result = ToolResult::new(ToolName::new("xml_tool")).success(
            json!({
                "text": "Special chars: < > & ' \"",
                "nested": {
                    "html": "<div>Test</div>"
                }
            })
            .to_string(),
        );
        assert_snapshot!(result);
    }

    #[test]
    fn test_display_minimal() {
        let result = ToolResult::new(ToolName::new("test_tool"));
        assert_snapshot!(result.to_string());
    }

    #[test]
    fn test_display_full() {
        let result = ToolResult::new(ToolName::new("complex_tool"))
            .call_id(ToolCallId::new("123"))
            .success(
                json!({
                    "user": "John Doe",
                    "age": 42,
                    "address": [{"city": "New York"}, {"city": "Los Angeles"}]
                })
                .to_string(),
            );
        assert_snapshot!(result.to_string());
    }

    #[test]
    fn test_display_special_chars() {
        let result = ToolResult::new(ToolName::new("xml_tool")).success(
            json!({
                "text": "Special chars: < > & ' \"",
                "nested": {
                    "html": "<div>Test</div>"
                }
            })
            .to_string(),
        );
        assert_snapshot!(result.to_string());
    }

    #[test]
    fn test_success_and_failure_content() {
        let success = ToolResult::new(ToolName::new("test_tool")).success("success message");
        assert!(!success.is_error);
        assert_eq!(success.content, "success message");

        let failure =
            ToolResult::new(ToolName::new("test_tool")).failure(anyhow::anyhow!("error message"));
        assert!(failure.is_error);
        assert_eq!(failure.content, "\nERROR:\nCaused by: error message\n");
    }

    #[test]
    fn test_roundtrip_toolresponse() {
        let original = ToolResult::new(ToolName::new("test_roundtrip")).success("Hello round-trip");
        let fm = original.to_front_matter();
        let parsed = ToolResult::from_frontmatter(&fm).unwrap();
        assert_eq!(parsed, original);
    }
}
