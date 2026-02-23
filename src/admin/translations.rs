//! Admin UI translation loading: compiled-in English + config dir overlay.

use std::collections::HashMap;
use std::path::Path;

static DEFAULT_EN: &str = include_str!("../../translations/en.json");

/// Holds resolved translation strings for a single locale.
pub struct Translations {
    strings: HashMap<String, String>,
}

impl Translations {
    /// Load translations: compiled-in en.json as base, overlaid with
    /// `<config_dir>/translations/<locale>.json` if it exists.
    pub fn load(config_dir: &Path, locale: &str) -> Self {
        let mut strings: HashMap<String, String> =
            serde_json::from_str(DEFAULT_EN).unwrap_or_default();

        // Overlay with config dir translations/{locale}.json if exists
        let locale_file = config_dir.join("translations").join(format!("{}.json", locale));
        if locale_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&locale_file) {
                if let Ok(overrides) = serde_json::from_str::<HashMap<String, String>>(&content) {
                    strings.extend(overrides);
                }
            }
        }

        Translations { strings }
    }

    /// Get a translated string by key. Returns the key itself if not found.
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.strings.get(key).map(|s| s.as_str()).unwrap_or(key)
    }

    /// Get a translated string and interpolate `{{var}}` placeholders with the given params.
    pub fn get_interpolated(&self, key: &str, params: &HashMap<String, String>) -> String {
        let template = self.get(key);
        if params.is_empty() {
            return template.to_string();
        }
        let mut result = template.to_string();
        for (k, v) in params {
            result = result.replace(&format!("{{{{{}}}}}", k), v);
        }
        result
    }
}
