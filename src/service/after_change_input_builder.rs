use std::collections::HashMap;

use serde_json::Value;

use crate::core::Document;

use super::AfterChangeInput;

/// Builder for [`AfterChangeInput`]. Created via [`AfterChangeInput::builder`].
pub(crate) struct AfterChangeInputBuilder<'a> {
    pub(super) slug: &'a str,
    pub(super) operation: &'a str,
    pub(super) locale: Option<String>,
    pub(super) is_draft: bool,
    pub(super) req_context: HashMap<String, Value>,
    pub(super) user: Option<&'a Document>,
    pub(super) ui_locale: Option<&'a str>,
}

impl<'a> AfterChangeInputBuilder<'a> {
    pub fn new(slug: &'a str, operation: &'a str) -> Self {
        Self {
            slug,
            operation,
            locale: None,
            is_draft: false,
            req_context: HashMap::new(),
            user: None,
            ui_locale: None,
        }
    }

    pub fn locale(mut self, locale: Option<String>) -> Self {
        self.locale = locale;
        self
    }

    pub fn draft(mut self, is_draft: bool) -> Self {
        self.is_draft = is_draft;
        self
    }

    pub fn req_context(mut self, req_context: HashMap<String, Value>) -> Self {
        self.req_context = req_context;
        self
    }

    pub fn user(mut self, user: Option<&'a Document>) -> Self {
        self.user = user;
        self
    }

    pub fn ui_locale(mut self, ui_locale: Option<&'a str>) -> Self {
        self.ui_locale = ui_locale;
        self
    }

    pub fn build(self) -> AfterChangeInput<'a> {
        AfterChangeInput {
            slug: self.slug,
            operation: self.operation,
            locale: self.locale,
            is_draft: self.is_draft,
            req_context: self.req_context,
            user: self.user,
            ui_locale: self.ui_locale,
        }
    }
}
