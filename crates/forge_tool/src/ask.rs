use forge_domain::{NamedTool, PermissionRequest, ToolCallService, ToolDescription, ToolName, ToolPermissions};
use forge_tool_macros::ToolDescription;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct AskFollowUpQuestionInput {
    /// The question to ask the user. This should be a clear, specific question
    /// that addresses the information you need.
    pub question: String,
}

/// Ask the user a question to gather additional information needed to complete
/// the task. This tool should be used when you encounter ambiguities, need
/// clarification, or require more details to proceed effectively. It allows for
/// interactive problem-solving by enabling direct communication with the user.
#[derive(ToolDescription)]
pub struct AskFollowUpQuestion;

impl NamedTool for AskFollowUpQuestion {
    fn tool_name(&self) -> ToolName {
        ToolName::new("ask_follow_up_question")
    }
}

impl ToolPermissions for AskFollowUpQuestion {
    fn required_permissions(&self) -> Vec<forge_domain::Permission> {
        vec![]
    }
    
}

#[async_trait::async_trait]
impl ToolCallService for AskFollowUpQuestion {
    type Input = AskFollowUpQuestionInput;
    type Output = String;

    async fn call(&self, input: Self::Input) -> Result<Self::Output, String> {
        Ok(format!("Question: {}", input.question))
    }
    async fn permission_check(&self, _input: Self::Input) -> PermissionRequest {
        PermissionRequest::new(self.required_permissions(), None)
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn test_ask_followup_question() {
        let ask = AskFollowUpQuestion;
        let result = ask
            .call(AskFollowUpQuestionInput { question: "What is your favorite color?".to_string() })
            .await
            .unwrap();

        assert_eq!(result, "Question: What is your favorite color?");
    }

    #[test]
    fn test_description() {
        assert!(AskFollowUpQuestion.description().len() > 100)
    }
}
