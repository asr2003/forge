#[derive(Debug, schemars::JsonSchema, serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq, Hash)]
pub struct Attachment {
    pub data: String,
}
