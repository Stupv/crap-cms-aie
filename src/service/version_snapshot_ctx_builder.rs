//! Builder for [`VersionSnapshotCtx`].

use crate::core::{collection::VersionsConfig, field::FieldDefinition};

use super::versions::VersionSnapshotCtx;

/// Builder for [`VersionSnapshotCtx`]. Created via [`VersionSnapshotCtx::builder`].
pub(crate) struct VersionSnapshotCtxBuilder<'a> {
    table: &'a str,
    parent_id: &'a str,
    fields: &'a [FieldDefinition],
    versions: Option<&'a VersionsConfig>,
    has_drafts: bool,
}

impl<'a> VersionSnapshotCtxBuilder<'a> {
    pub(crate) fn new(table: &'a str, parent_id: &'a str) -> Self {
        Self {
            table,
            parent_id,
            fields: &[],
            versions: None,
            has_drafts: false,
        }
    }

    pub fn fields(mut self, fields: &'a [FieldDefinition]) -> Self {
        self.fields = fields;
        self
    }

    pub fn versions(mut self, versions: Option<&'a VersionsConfig>) -> Self {
        self.versions = versions;
        self
    }

    pub fn has_drafts(mut self, has_drafts: bool) -> Self {
        self.has_drafts = has_drafts;
        self
    }

    pub fn build(self) -> VersionSnapshotCtx<'a> {
        VersionSnapshotCtx {
            table: self.table,
            parent_id: self.parent_id,
            fields: self.fields,
            versions: self.versions,
            has_drafts: self.has_drafts,
        }
    }
}
