//! Shared `ColorfulTheme` factory for all dialoguer prompts.

use console::style;
use dialoguer::theme::ColorfulTheme;

/// Build the standard Crap CMS theme for all dialoguer prompts.
///
/// Uses `?` as the prompt prefix (cyan), `✓` for success (green), `✗` for errors (red).
pub fn crap_theme() -> ColorfulTheme {
    ColorfulTheme {
        prompt_prefix: style("?".to_string()).cyan().bold(),
        success_prefix: style("✓".to_string()).green().bold(),
        error_prefix: style("✗".to_string()).red().bold(),
        ..ColorfulTheme::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_creates_without_panic() {
        let theme = crap_theme();
        // Verify the theme has the expected prefix strings
        assert!(theme.prompt_prefix.to_string().contains('?'));
        assert!(theme.success_prefix.to_string().contains('✓'));
        assert!(theme.error_prefix.to_string().contains('✗'));
    }
}
