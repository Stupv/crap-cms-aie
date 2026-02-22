/**
 * Crap CMS Web Components
 *
 * Native custom elements for interactive UI behavior.
 * No build step — plain JS, loaded via <script defer> in the base layout.
 * Uses Shadow DOM for style encapsulation.
 *
 * Components:
 * - <crap-toast>   Toast notifications (fixed bottom-right, auto-dismiss)
 * - <crap-confirm> Confirmation dialog for destructive actions
 */

/* ── <crap-toast> ─────────────────────────────────────────────── */

/**
 * Toast notification container element.
 *
 * Renders fixed-position toast messages with type-based coloring
 * and auto-dismiss. Listens for HTMX responses with `X-Crap-Toast`
 * header to auto-show server-driven toasts.
 *
 * @example HTML usage:
 * <crap-toast></crap-toast>
 *
 * @example JS API:
 * window.CrapToast.show('Item created', 'success');
 * window.CrapToast.show('Something went wrong', 'error', 5000);
 *
 * @example Server-driven (via response header):
 * X-Crap-Toast: {"message": "Saved", "type": "success"}
 * X-Crap-Toast: Plain text message (defaults to success)
 */
class CrapToast extends HTMLElement {
  constructor() {
    super();
    this.attachShadow({ mode: 'open' });
    this.shadowRoot.innerHTML = `
      <style>
        :host {
          position: fixed;
          bottom: 1.5rem;
          right: 1.5rem;
          z-index: 10000;
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
          pointer-events: none;
        }
        .toast {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.75rem 1.25rem;
          border-radius: 8px;
          font-family: inherit;
          font-size: 0.875rem;
          font-weight: 500;
          color: #fff;
          background: #1f2937;
          box-shadow: 0 8px 24px rgba(0, 0, 0, 0.15);
          pointer-events: auto;
          cursor: pointer;
          animation: toast-in 0.3s ease forwards;
          max-width: 380px;
        }
        .toast.removing {
          animation: toast-out 0.25s ease forwards;
        }
        .toast--success { background: #16a34a; }
        .toast--error   { background: #dc2626; }
        .toast--info    { background: #1677ff; }
        @keyframes toast-in {
          from { opacity: 0; transform: translateY(12px) scale(0.96); }
          to   { opacity: 1; transform: translateY(0) scale(1); }
        }
        @keyframes toast-out {
          from { opacity: 1; transform: translateY(0) scale(1); }
          to   { opacity: 0; transform: translateY(-8px) scale(0.96); }
        }
      </style>
    `;
  }

  /**
   * Display a toast notification.
   *
   * @param {string} message - Text content to display.
   * @param {'success' | 'error' | 'info'} [type='info'] - Visual style variant.
   * @param {number} [duration=3000] - Auto-dismiss delay in ms. Use 0 for persistent.
   * @returns {void}
   */
  show(message, type = 'info', duration = 3000) {
    /** @type {HTMLDivElement} */
    const toast = document.createElement('div');
    toast.className = `toast toast--${type}`;
    toast.textContent = message;
    this.shadowRoot.appendChild(toast);

    /** @type {() => void} */
    const remove = () => {
      toast.classList.add('removing');
      toast.addEventListener('animationend', () => toast.remove(), { once: true });
    };

    if (duration > 0) {
      setTimeout(remove, duration);
    }

    toast.addEventListener('click', remove);
  }

  /**
   * Lifecycle callback — registers HTMX event listener for server-driven toasts.
   *
   * Listens for `htmx:afterRequest` events. If the response includes an
   * `X-Crap-Toast` header, parses it and shows a toast. The header value
   * can be a JSON object `{"message": "...", "type": "..."}` or a plain string.
   *
   * @returns {void}
   */
  connectedCallback() {
    /** @param {CustomEvent} e - HTMX afterRequest event */
    const handler = (e) => {
      const xhr = /** @type {XMLHttpRequest | null} */ (e.detail.xhr);
      if (!xhr) return;

      const header = xhr.getResponseHeader('X-Crap-Toast');
      if (header) {
        try {
          /** @type {{ message: string, type?: string }} */
          const data = JSON.parse(header);
          this.show(data.message, /** @type {any} */ (data.type || 'success'));
        } catch {
          this.show(header, 'success');
        }
      }
    };

    document.body.addEventListener('htmx:afterRequest', handler);
  }
}

customElements.define('crap-toast', CrapToast);

/**
 * Global toast API.
 *
 * Convenience wrapper that finds or creates the <crap-toast> element
 * and delegates to its `show()` method.
 *
 * @namespace
 */
window.CrapToast = {
  /**
   * Show a toast notification from anywhere.
   *
   * @param {string} message - Text content to display.
   * @param {'success' | 'error' | 'info'} [type='info'] - Visual style variant.
   * @param {number} [duration=3000] - Auto-dismiss delay in ms.
   * @returns {void}
   */
  show(message, type = 'info', duration = 3000) {
    /** @type {CrapToast | null} */
    let el = document.querySelector('crap-toast');
    if (!el) {
      el = /** @type {CrapToast} */ (document.createElement('crap-toast'));
      document.body.appendChild(el);
    }
    el.show(message, type, duration);
  },
};


/* ── <crap-confirm> ───────────────────────────────────────────── */

/**
 * Confirmation dialog that wraps destructive actions.
 *
 * Intercepts `submit` events from child forms, shows a native `<dialog>`
 * with the configured message, and only allows the submission through
 * if the user confirms.
 *
 * @attr {string} message - Confirmation prompt text (default: "Are you sure?").
 *
 * @example
 * <crap-confirm message="Delete this item permanently?">
 *   <form method="post" action="/delete/123">
 *     <button type="submit" class="button button--danger">Delete</button>
 *   </form>
 * </crap-confirm>
 */
class CrapConfirm extends HTMLElement {
  constructor() {
    super();

    /**
     * Flag to bypass interception on confirmed re-submit.
     * @type {boolean}
     * @private
     */
    this._confirmed = false;

    /**
     * Reference to the form that triggered the confirmation.
     * @type {HTMLFormElement | null}
     * @private
     */
    this._pendingForm = null;

    this.attachShadow({ mode: 'open' });
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: contents; }
        dialog {
          border: none;
          border-radius: 12px;
          padding: 0;
          max-width: 400px;
          width: 90vw;
          box-shadow: 0 16px 48px rgba(0, 0, 0, 0.2);
          font-family: inherit;
        }
        dialog::backdrop {
          background: rgba(0, 0, 0, 0.4);
        }
        .dialog__body {
          padding: 1.5rem;
        }
        .dialog__body p {
          margin: 0;
          font-size: 0.95rem;
          color: rgba(0, 0, 0, 0.8);
          line-height: 1.5;
        }
        .dialog__actions {
          display: flex;
          justify-content: flex-end;
          gap: 0.5rem;
          padding: 0 1.5rem 1.5rem;
        }
        button {
          font-family: inherit;
          font-size: 0.875rem;
          font-weight: 500;
          padding: 0.5rem 1rem;
          border-radius: 6px;
          border: none;
          cursor: pointer;
          transition: background 0.15s ease;
        }
        .btn-cancel {
          background: transparent;
          color: rgba(0, 0, 0, 0.65);
          border: 1px solid #d9d9d9;
        }
        .btn-cancel:hover { background: rgba(0, 0, 0, 0.04); }
        .btn-confirm {
          background: #dc2626;
          color: #fff;
        }
        .btn-confirm:hover { background: #ef4444; }
      </style>
      <slot></slot>
      <dialog>
        <div class="dialog__body">
          <p></p>
        </div>
        <div class="dialog__actions">
          <button class="btn-cancel" type="button">Cancel</button>
          <button class="btn-confirm" type="button">Confirm</button>
        </div>
      </dialog>
    `;
  }

  /**
   * Lifecycle callback — wires up submit interception and dialog controls.
   *
   * Flow:
   * 1. Child form submits → intercepted, dialog shown.
   * 2. User clicks Cancel → dialog closes, form is not submitted.
   * 3. User clicks Confirm → dialog closes, `_confirmed` flag set,
   *    form re-submitted via `requestSubmit()` (preserves HTMX attributes).
   * 4. Re-submit fires submit event again → `_confirmed` flag lets it through.
   *
   * @returns {void}
   */
  connectedCallback() {
    /** @type {HTMLDialogElement} */
    const dialog = this.shadowRoot.querySelector('dialog');
    /** @type {HTMLParagraphElement} */
    const messageEl = this.shadowRoot.querySelector('.dialog__body p');
    /** @type {HTMLButtonElement} */
    const cancelBtn = this.shadowRoot.querySelector('.btn-cancel');
    /** @type {HTMLButtonElement} */
    const confirmBtn = this.shadowRoot.querySelector('.btn-confirm');

    this.addEventListener('submit', (e) => {
      if (this._confirmed) {
        this._confirmed = false;
        return; // let re-submit through
      }
      e.preventDefault();
      e.stopPropagation();
      this._pendingForm = /** @type {HTMLFormElement} */ (e.target);
      messageEl.textContent = this.getAttribute('message') || 'Are you sure?';
      dialog.showModal();
    });

    cancelBtn.addEventListener('click', () => {
      this._pendingForm = null;
      dialog.close();
    });

    confirmBtn.addEventListener('click', () => {
      dialog.close();
      if (this._pendingForm) {
        const form = this._pendingForm;
        this._pendingForm = null;
        this._confirmed = true;
        form.requestSubmit();
      }
    });
  }
}

customElements.define('crap-confirm', CrapConfirm);

/* ── Array field repeater ──────────────────────────────────────── */

/**
 * Add a new row to an array field repeater.
 * Clones the <template> for the field, replaces __INDEX__ placeholders
 * with the next row index, and appends to the rows container.
 *
 * @param {string} fieldName - The array field name (matches data-field-name on the fieldset)
 */
function addArrayRow(fieldName) {
  const template = document.getElementById(`array-template-${fieldName}`);
  const container = document.getElementById(`array-rows-${fieldName}`);
  if (!template || !container) return;

  const nextIndex = container.children.length;
  const clone = template.content.cloneNode(true);

  // Replace all __INDEX__ placeholders in the cloned content
  const html = /** @type {HTMLElement} */ (clone.firstElementChild);
  if (html) {
    html.setAttribute('data-row-index', String(nextIndex));
    html.querySelectorAll('input, select, textarea').forEach(
      /** @param {HTMLInputElement} input */ (input) => {
        if (input.name) {
          input.name = input.name.replace(/__INDEX__/g, String(nextIndex));
        }
      }
    );
  }

  container.appendChild(clone);
}

/**
 * Remove an array row from the repeater.
 * Re-indexes remaining rows so form keys stay sequential.
 *
 * @param {HTMLButtonElement} btn - The remove button inside the row
 */
function removeArrayRow(btn) {
  const row = btn.closest('.form__array-row');
  if (!row) return;

  const container = row.parentElement;
  row.remove();

  // Re-index remaining rows
  if (container) {
    Array.from(container.children).forEach(
      /** @param {Element} child @param {number} idx */
      (child, idx) => {
        child.setAttribute('data-row-index', String(idx));
        child.querySelectorAll('input, select, textarea').forEach(
          /** @param {HTMLInputElement} input */ (input) => {
            if (input.name) {
              input.name = input.name.replace(/\[\d+\]/, `[${idx}]`);
            }
          }
        );
      }
    );
  }
}
