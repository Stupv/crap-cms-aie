use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::upload::{FormatResult, SizeResultBuilder};

/// Output metadata for one generated image size.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeResult {
    pub url: String,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub formats: HashMap<String, FormatResult>,
}

impl SizeResult {
    pub fn builder(url: impl Into<String>) -> SizeResultBuilder {
        SizeResultBuilder::new(url)
    }
}
