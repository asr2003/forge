use forge_domain::{ToolCallService, ToolDescription};
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

#[async_trait::async_trait]
impl ToolCallService for AskFollowUpQuestion {
    type Input = AskFollowUpQuestionInput;
    type Output = String;

    async fn call(&self, input: Self::Input) -> Result<Self::Output, String> {
        Ok(format!("Question: {}", input.question))
    }
}

/// Select one option from a list of choices.
pub async fn select(_message: &str, options: &[&str]) -> Result<String, String> {
    // When running tests, delay to allow timeout testing
    #[cfg(test)]
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // For testing purposes, simulate a selection
    Ok(options[0].to_string())
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

    #[tokio::test]
    async fn test_select() {
        let result = select(
            "Choose an option:",
            &["A", "B", "C"]
        ).await.unwrap();
        
        assert_eq!(result, "A");
    }
}