/**
 * Display conditions — conditional field visibility.
 *
 * Supports two modes:
 * - Client-side (data-condition): JSON condition table, evaluated instantly.
 * - Server-side (data-condition-ref): Lua function ref, debounced POST.
 */

import { registerInit } from './actions.js';

/**
 * Check if a value is truthy (mirrors Rust condition_is_truthy).
 *
 * @param {*} val
 * @returns {boolean}
 */
function conditionIsTruthy(val) {
  if (val == null || val === '' || val === false) return false;
  if (Array.isArray(val)) return val.length > 0;
  return true;
}

/**
 * Evaluate a condition table against form data.
 * Mirrors the Rust `evaluate_condition_table` function.
 *
 * @param {Object|Array} condition
 * @param {Object} formData
 * @returns {boolean}
 */
function evaluateCondition(condition, formData) {
  if (Array.isArray(condition)) {
    return condition.every(function(c) { return evaluateCondition(c, formData); });
  }
  var fieldVal = formData[condition.field];
  if (fieldVal === undefined) fieldVal = '';
  if ('equals' in condition) return fieldVal === condition.equals;
  if ('not_equals' in condition) return fieldVal !== condition.not_equals;
  if ('in' in condition) return condition['in'].includes(fieldVal);
  if ('not_in' in condition) return !condition['not_in'].includes(fieldVal);
  if (condition.is_truthy) return conditionIsTruthy(fieldVal);
  if (condition.is_falsy) return !conditionIsTruthy(fieldVal);
  return true;
}

/**
 * Collect current form field values as a flat object.
 *
 * @param {HTMLFormElement} form
 * @returns {Object<string, string>}
 */
function collectFormData(form) {
  var data = {};
  var fd = new FormData(form);
  for (var pair of fd.entries()) {
    var key = pair[0];
    var val = pair[1];
    if (key.startsWith('_')) continue;
    data[key] = /** @type {string} */ (val);
  }
  // Unchecked checkboxes are absent from FormData
  form.querySelectorAll('input[type="checkbox"]').forEach(
    /** @param {HTMLInputElement} cb */ function(cb) {
      if (!cb.name.startsWith('_') && !(cb.name in data)) {
        data[cb.name] = cb.checked ? 'on' : '';
      }
    }
  );
  return data;
}

/**
 * Extract watched field names from a condition table.
 *
 * @param {Object|Array} condition
 * @param {Set<string>} set
 */
function extractWatchedFields(condition, set) {
  if (Array.isArray(condition)) {
    condition.forEach(function(c) { extractWatchedFields(c, set); });
  } else if (condition && condition.field) {
    set.add(condition.field);
  }
}

/**
 * Initialize display conditions on the edit form.
 */
function initDisplayConditions() {
  var form = /** @type {HTMLFormElement | null} */ (document.getElementById('edit-form'));
  if (!form) return;

  var clientFields = form.querySelectorAll('[data-condition]');
  var serverFields = form.querySelectorAll('[data-condition-ref]');

  if (clientFields.length === 0 && serverFields.length === 0) return;

  // --- Client-side conditions (instant) ---

  /** @type {Set<string>} */
  var watchedFields = new Set();
  clientFields.forEach(function(el) {
    try {
      var cond = JSON.parse(/** @type {HTMLElement} */ (el).dataset.condition);
      extractWatchedFields(cond, watchedFields);
    } catch (e) { /* skip malformed JSON */ }
  });

  function runClientConditions() {
    var data = collectFormData(form);
    clientFields.forEach(function(el) {
      try {
        var cond = JSON.parse(/** @type {HTMLElement} */ (el).dataset.condition);
        var visible = evaluateCondition(cond, data);
        el.classList.toggle('form__field--hidden', !visible);
      } catch (e) { /* skip */ }
    });
  }

  watchedFields.forEach(function(fieldName) {
    var input = form.querySelector('[name="' + fieldName + '"]');
    if (input) {
      input.addEventListener('input', runClientConditions);
      input.addEventListener('change', runClientConditions);
    }
  });

  // --- Server-side conditions (debounced) ---

  if (serverFields.length > 0) {
    /** @type {number | null} */
    var serverTimer = null;
    var slug = form.dataset.collectionSlug || '';

    function runServerConditions() {
      var data = collectFormData(form);
      /** @type {Object<string, string>} */
      var refs = {};
      serverFields.forEach(function(el) {
        var name = /** @type {HTMLElement} */ (el).dataset.fieldName;
        var ref = /** @type {HTMLElement} */ (el).dataset.conditionRef;
        if (name && ref) refs[name] = ref;
      });

      fetch('/admin/collections/' + slug + '/evaluate-conditions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ form_data: data, conditions: refs }),
      })
      .then(function(r) { return r.json(); })
      .then(function(result) {
        for (var fieldName in result) {
          var el = form.querySelector(
            '[data-field-name="' + fieldName + '"][data-condition-ref]'
          );
          if (el) el.classList.toggle('form__field--hidden', !result[fieldName]);
        }
      })
      .catch(function() { /* silent fail — keep current visibility */ });
    }

    /** @param {Event} _e */
    function debouncedServer(_e) {
      clearTimeout(serverTimer);
      serverTimer = setTimeout(runServerConditions, 300);
    }

    form.addEventListener('input', debouncedServer);
    form.addEventListener('change', debouncedServer);
  }
}

registerInit(initDisplayConditions);
