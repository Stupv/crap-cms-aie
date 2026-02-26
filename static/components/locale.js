/**
 * Locale persistence — carries selected content locale across navigations.
 *
 * Stores the locale in sessionStorage. On HTMX navigations (full-page
 * swaps to body), the stored locale is automatically appended as
 * `?locale=XX` so it carries over to list pages, other collections, etc.
 */

const STORAGE_KEY = 'crap_locale';

/**
 * @returns {string | null}
 */
function getStoredLocale() {
  try { return sessionStorage.getItem(STORAGE_KEY); }
  catch { return null; }
}

/**
 * @param {string | null} locale
 */
function setStoredLocale(locale) {
  try {
    if (locale) sessionStorage.setItem(STORAGE_KEY, locale);
    else sessionStorage.removeItem(STORAGE_KEY);
  } catch { /* private browsing — ignore */ }
}

/**
 * Bind click handlers on locale selector links to save the chosen locale.
 */
function bindLocaleSelector() {
  document.querySelectorAll('.locale-selector__item').forEach(
    /** @param {HTMLAnchorElement} link */ (link) => {
      link.addEventListener('click', () => {
        const url = new URL(link.href, location.origin);
        const locale = url.searchParams.get('locale');
        setStoredLocale(locale);
      });
    }
  );
}

// Seed from current page's locale param on first load
const initial = new URLSearchParams(location.search).get('locale');
if (initial) setStoredLocale(initial);

document.addEventListener('DOMContentLoaded', bindLocaleSelector);
document.addEventListener('htmx:afterSettle', bindLocaleSelector);

/**
 * Before every HTMX request, inject the stored locale into the URL
 * if the request is a full-page navigation (target=body) and the URL
 * doesn't already have a `locale` param.
 */
document.body.addEventListener('htmx:configRequest', /** @param {CustomEvent} e */ (e) => {
  const locale = getStoredLocale();
  if (!locale) return;

  const detail = e.detail;
  if (detail.verb !== 'get') return;
  const target = detail.elt.getAttribute('hx-target') ||
                 detail.elt.closest('[hx-target]')?.getAttribute('hx-target');
  if (target !== 'body') return;

  const url = new URL(detail.path, location.origin);
  if (url.searchParams.has('locale')) return;

  url.searchParams.set('locale', locale);
  detail.path = url.pathname + url.search;
});
