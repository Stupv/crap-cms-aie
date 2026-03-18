#[allow(dead_code)]
mod helpers;
#[allow(dead_code)]
mod html;

mod html_auth;
mod html_crud;
mod html_forms;
mod html_globals;
mod html_locale;
mod html_nesting;
mod html_validation;
mod html_versions;

#[cfg(feature = "browser-tests")]
mod browser;
#[cfg(feature = "browser-tests")]
mod browser_array;
#[cfg(feature = "browser-tests")]
mod browser_tabs;
#[cfg(feature = "browser-tests")]
mod browser_toast;
#[cfg(feature = "browser-tests")]
mod browser_validation;
