/**
 * Theme switching — persistence, application, and picker UI.
 *
 * Sets `data-theme` attribute on <html> and persists to localStorage.
 * Re-inits picker on htmx:afterSettle because HTMX body swaps destroy
 * the picker DOM.
 *
 * @namespace window.CrapTheme
 */

import { registerInit } from './actions.js';

window.CrapTheme = {
  /** @type {string} localStorage key */
  _key: 'crap-theme',

  /**
   * Get the current theme name from localStorage.
   * @returns {string} Theme name or '' for default light.
   */
  get() {
    return localStorage.getItem(this._key) || '';
  },

  /**
   * Apply a theme to the document without saving.
   * @param {string} theme - Theme name ('' for light default).
   */
  apply(theme) {
    if (theme) {
      document.documentElement.setAttribute('data-theme', theme);
    } else {
      document.documentElement.removeAttribute('data-theme');
    }
  },

  /**
   * Set and persist a theme.
   * @param {string} theme - Theme name ('' for light default).
   */
  set(theme) {
    if (theme) {
      localStorage.setItem(this._key, theme);
    } else {
      localStorage.removeItem(this._key);
    }
    this.apply(theme);
  },

  /**
   * Initialize theme picker UI. Safe to call multiple times
   * (idempotent — skips already-initialized pickers).
   */
  initPicker() {
    document.querySelectorAll('[data-theme-picker]').forEach((picker) => {
      if (/** @type {HTMLElement} */ (picker).dataset.themeInit) return;
      /** @type {HTMLElement} */ (picker).dataset.themeInit = '1';

      const toggle = picker.querySelector('[data-theme-toggle]');
      const dropdown = picker.querySelector('[data-theme-dropdown]');
      if (!toggle || !dropdown) return;

      /** Update active state on options */
      const updateActive = () => {
        const current = this.get();
        dropdown.querySelectorAll('[data-theme-value]').forEach((btn) => {
          const val = /** @type {HTMLElement} */ (btn).dataset.themeValue;
          btn.classList.toggle('theme-picker__option--active', val === current);
        });
      };

      toggle.addEventListener('click', (e) => {
        e.stopPropagation();
        dropdown.classList.toggle('theme-picker__dropdown--open');
        updateActive();
      });

      dropdown.addEventListener('click', (e) => {
        const btn = /** @type {HTMLElement} */ (e.target).closest('[data-theme-value]');
        if (!btn) return;
        this.set(/** @type {HTMLElement} */ (btn).dataset.themeValue || '');
        dropdown.classList.remove('theme-picker__dropdown--open');
        updateActive();
      });

      // Close on outside click
      document.addEventListener('click', (e) => {
        if (!picker.contains(/** @type {Node} */ (e.target))) {
          dropdown.classList.remove('theme-picker__dropdown--open');
        }
      });
    });
  },
};

registerInit(() => {
  window.CrapTheme.apply(window.CrapTheme.get());
  window.CrapTheme.initPicker();
});
