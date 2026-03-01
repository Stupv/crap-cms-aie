/**
 * Tab field switching.
 *
 * Registers a `switch-tab` action via the delegation system.
 * State persistence is handled by scroll.js (snapshot before form save,
 * restore after same-page HTMX settle).
 */

import { registerAction } from './actions.js';

/**
 * Switch to a tab panel by index.
 *
 * @param {HTMLElement} button - The tab button clicked.
 * @param {string} index - The tab panel index.
 */
function switchTab(button, index) {
  const tabs = button.closest('.form__tabs');
  if (!tabs) return;
  tabs.querySelectorAll('.form__tabs-tab').forEach(t => {
    t.classList.remove('form__tabs-tab--active');
    t.setAttribute('aria-selected', 'false');
  });
  tabs.querySelectorAll('.form__tabs-panel').forEach(p => p.classList.add('form__tabs-panel--hidden'));
  button.classList.add('form__tabs-tab--active');
  button.setAttribute('aria-selected', 'true');
  tabs.querySelector(`[data-tab-panel="${index}"]`).classList.remove('form__tabs-panel--hidden');
}

registerAction('switch-tab', (el) => switchTab(el, el.dataset.tabIndex));
