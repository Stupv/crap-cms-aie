/**
 * Event delegation and centralized init system for admin UI.
 *
 * Provides three registration APIs:
 * - `registerAction(name, handler)` — click delegation via `data-action` attributes
 * - `registerDrag(handlers)` — drag-and-drop delegation
 * - `registerInit(fn)` — init functions that run on DOMContentLoaded + htmx:afterSettle
 *
 * Modules call these instead of adding their own `document.addEventListener` pairs.
 * Overriding `index.js` to remove an import prevents that module's actions, drag
 * handlers, and init functions from registering — single file controls everything.
 *
 * @module actions
 */

/* ── Click delegation ────────────────────────────────────────── */

/** @type {Map<string, (el: HTMLElement, e: MouseEvent) => void>} */
const actions = new Map();

/**
 * Register a click handler for a `data-action` name.
 *
 * Template elements use `data-action="name"` and the delegation listener
 * dispatches to the matching handler, passing the element and event.
 *
 * @param {string} name - Action name (matches `data-action` attribute value).
 * @param {(el: HTMLElement, e: MouseEvent) => void} handler
 */
export function registerAction(name, handler) {
  actions.set(name, handler);
}

document.addEventListener('click', (e) => {
  const el = /** @type {HTMLElement|null} */ (
    /** @type {HTMLElement} */ (e.target).closest('[data-action]')
  );
  if (!el) return;
  const name = el.dataset.action;
  if (name === 'noop') return;          // consume click, do nothing
  const handler = actions.get(name);
  if (handler) handler(el, e);
});

/* ── Drag delegation ─────────────────────────────────────────── */

/**
 * @typedef {Object} DragHandlers
 * @property {(el: HTMLElement, e: DragEvent) => void} [start]
 * @property {(e: DragEvent) => void} [end]
 * @property {(container: HTMLElement, e: DragEvent) => void} [over]
 * @property {(container: HTMLElement, e: DragEvent) => void} [drop]
 */

/** @type {DragHandlers} */
let dragHandlers = {};

/**
 * Register drag-and-drop handlers for `[draggable][data-drag]` elements
 * inside `.form__array-rows` containers.
 *
 * @param {DragHandlers} handlers
 */
export function registerDrag(handlers) {
  dragHandlers = handlers;
}

document.addEventListener('dragstart', (e) => {
  const el = /** @type {HTMLElement|null} */ (
    /** @type {HTMLElement} */ (e.target).closest('[draggable][data-drag]')
  );
  if (el && dragHandlers.start) dragHandlers.start(el, /** @type {DragEvent} */ (e));
});

document.addEventListener('dragend', (e) => {
  if (dragHandlers.end) dragHandlers.end(/** @type {DragEvent} */ (e));
});

document.addEventListener('dragover', (e) => {
  const container = /** @type {HTMLElement|null} */ (
    /** @type {HTMLElement} */ (e.target).closest('.form__array-rows')
  );
  if (container && dragHandlers.over) dragHandlers.over(container, /** @type {DragEvent} */ (e));
});

document.addEventListener('drop', (e) => {
  const container = /** @type {HTMLElement|null} */ (
    /** @type {HTMLElement} */ (e.target).closest('.form__array-rows')
  );
  if (container && dragHandlers.drop) dragHandlers.drop(container, /** @type {DragEvent} */ (e));
});

/* ── Centralized init ────────────────────────────────────────── */

/** @type {Array<() => void>} */
const initFns = [];

/**
 * Register an init function that runs on DOMContentLoaded and htmx:afterSettle.
 *
 * Modules call this instead of adding their own listener pairs. If a module
 * isn't imported (because the user overrode index.js), its init never registers.
 *
 * @param {() => void} fn
 */
export function registerInit(fn) {
  initFns.push(fn);
}

/** Run all registered init functions. */
function runInits() {
  for (const fn of initFns) fn();
}

document.addEventListener('DOMContentLoaded', runInits);
document.addEventListener('htmx:afterSettle', runInits);
