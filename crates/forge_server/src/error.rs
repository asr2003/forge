use std::fmt::{Debug, Display, Formatter};

use derive_more::derive::{Display, From};
use derive_setters::Setters;
use serde::Serialize;

#[derive(Display, From)]
pub enum Error {
    // TODO: drop `Custom` because its too generic
    Custom(String),
    Provider(forge_provider::Error),
    IO(std::io::Error),
    Var(std::env::VarError),
    Serde(serde_json::Error),
    EmptyResponse,
    Walk(forge_walker::Error),
    Env(forge_env::Error),
    ToolCallMissingName,
    Handlebars(handlebars::RenderError),
}

pub type Result<T> = std::result::Result<T, Error>;

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&Errata::from(self), f)
    }
}

#[derive(Clone, Setters, Serialize, PartialEq, Eq)]
pub struct Errata {
    pub message: String,
    #[setters(strip_option, into)]
    pub description: Option<String>,
}

impl Errata {
    pub fn new(title: impl Into<String>) -> Self {
        Self { message: title.into(), description: None }
    }
}

impl Display for Errata {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Debug for Errata {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(desc) = &self.description {
            if !desc.trim().is_empty() {
                write!(f, "\n{}", desc)?;
            }
        }
        Ok(())
    }
}

impl From<&Error> for Errata {
    fn from(error: &Error) -> Self {
        Errata::new(error.to_string())
    }
}

#[cfg(test)]
mod tests {

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_simple_error() {
        let error = Errata::new("Something went wrong");
        assert_eq!(format!("{:?}", error), "Something went wrong");
    }

    #[test]
    fn test_error_with_description() {
        let error = Errata::new("Invalid input").description("Expected a number");
        assert_eq!(format!("{:?}", error), "Invalid input\nExpected a number");
    }
}
