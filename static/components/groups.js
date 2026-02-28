/**
 * Collapsible group fields.
 *
 * Persists collapsed state to localStorage keyed by group name.
 * `toggleGroup` is exported for use as a global (called from inline onclick).
 */

/**
 * Toggle a group fieldset's collapsed state.
 *
 * @param {HTMLButtonElement} btn - The toggle button inside the legend.
 */
export function toggleGroup(btn) {
  const fieldset = btn.closest('[data-collapsible]');
  if (!fieldset) return;
  const cls = fieldset.classList.contains('form__collapsible')
    ? 'form__collapsible--collapsed'
    : 'form__group--collapsed';
  fieldset.classList.toggle(cls);
  const name = fieldset.getAttribute('data-group-name');
  if (name) {
    const key = 'crap-group-' + name;
    if (fieldset.classList.contains(cls)) {
      localStorage.setItem(key, '1');
    } else {
      localStorage.removeItem(key);
    }
  }
}

/**
 * Restore group collapsed states from localStorage.
 */
function restoreGroupStates() {
  document.querySelectorAll('[data-collapsible][data-group-name]').forEach(
    /** @param {HTMLElement} fieldset */ (fieldset) => {
      const name = fieldset.getAttribute('data-group-name');
      if (!name) return;
      const key = 'crap-group-' + name;
      const stored = localStorage.getItem(key);
      const cls = fieldset.classList.contains('form__collapsible')
        ? 'form__collapsible--collapsed'
        : 'form__group--collapsed';
      if (stored === '1') {
        fieldset.classList.add(cls);
      } else if (stored === null) {
        // No override stored — keep the server-rendered default
      } else {
        fieldset.classList.remove(cls);
      }
    }
  );
}

document.addEventListener('DOMContentLoaded', restoreGroupStates);
document.addEventListener('htmx:afterSettle', restoreGroupStates);
