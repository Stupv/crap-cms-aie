use std::collections::HashMap;

use serde_json::Value;

use crate::db::LocaleContext;

use super::WriteInput;

/// Builder for [`WriteInput`]. Created via [`WriteInput::builder`].
pub struct WriteInputBuilder<'a> {
    pub(super) data: HashMap<String, String>,
    pub(super) join_data: &'a HashMap<String, Value>,
    pub(super) password: Option<&'a str>,
    pub(super) locale_ctx: Option<&'a LocaleContext>,
    pub(super) locale: Option<String>,
    pub(super) draft: bool,
    pub(super) ui_locale: Option<String>,
}

impl<'a> WriteInputBuilder<'a> {
    pub fn new(data: HashMap<String, String>, join_data: &'a HashMap<String, Value>) -> Self {
        Self {
            data,
            join_data,
            password: None,
            locale_ctx: None,
            locale: None,
            draft: false,
            ui_locale: None,
        }
    }

    pub fn password(mut self, password: Option<&'a str>) -> Self {
        self.password = password;
        self
    }

    pub fn locale_ctx(mut self, locale_ctx: Option<&'a LocaleContext>) -> Self {
        self.locale_ctx = locale_ctx;
        self
    }

    pub fn locale(mut self, locale: Option<String>) -> Self {
        self.locale = locale;
        self
    }

    pub fn draft(mut self, draft: bool) -> Self {
        self.draft = draft;
        self
    }

    pub fn ui_locale(mut self, ui_locale: Option<String>) -> Self {
        self.ui_locale = ui_locale;
        self
    }

    pub fn build(self) -> WriteInput<'a> {
        WriteInput {
            data: self.data,
            join_data: self.join_data,
            password: self.password,
            locale_ctx: self.locale_ctx,
            locale: self.locale,
            draft: self.draft,
            ui_locale: self.ui_locale,
        }
    }
}
