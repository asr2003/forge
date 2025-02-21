#[derive(Debug, schemars::JsonSchema, serde::Deserialize, serde::Serialize, Clone, PartialEq)]
pub struct Attachment {
    pub data: String,
}
