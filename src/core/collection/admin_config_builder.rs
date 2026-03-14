use crate::core::collection::AdminConfig;

/// Builder for [`AdminConfig`]. Created via [`AdminConfig::builder`].
pub struct AdminConfigBuilder {
    use_as_title: Option<String>,
    default_sort: Option<String>,
    hidden: bool,
    list_searchable_fields: Vec<String>,
}

impl AdminConfigBuilder {
    pub(crate) fn new() -> Self {
        Self {
            use_as_title: None,
            default_sort: None,
            hidden: false,
            list_searchable_fields: Vec::new(),
        }
    }

    pub fn use_as_title(mut self, v: Option<String>) -> Self {
        self.use_as_title = v;
        self
    }

    pub fn default_sort(mut self, v: Option<String>) -> Self {
        self.default_sort = v;
        self
    }

    pub fn hidden(mut self, v: bool) -> Self {
        self.hidden = v;
        self
    }

    pub fn list_searchable_fields(mut self, v: Vec<String>) -> Self {
        self.list_searchable_fields = v;
        self
    }

    pub fn build(self) -> AdminConfig {
        AdminConfig {
            use_as_title: self.use_as_title,
            default_sort: self.default_sort,
            hidden: self.hidden,
            list_searchable_fields: self.list_searchable_fields,
        }
    }
}
