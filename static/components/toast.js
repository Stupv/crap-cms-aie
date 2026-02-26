/**
 * <crap-toast> — Toast notification container.
 *
 * Renders fixed-position toast messages with type-based coloring
 * and auto-dismiss. Listens for HTMX responses with `X-Crap-Toast`
 * header to auto-show server-driven toasts.
 *
 * @example HTML:  <crap-toast></crap-toast>
 * @example JS:    window.CrapToast.show('Item created', 'success');
 * @example Header: X-Crap-Toast: {"message": "Saved", "type": "success"}
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
        .toast--success { background: var(--color-success, #16a34a); }
        .toast--error   { background: var(--color-danger, #dc2626); }
        .toast--info    { background: var(--color-primary, #1677ff); }
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

  /** @returns {void} */
  connectedCallback() {
    /** @param {CustomEvent} e - HTMX afterRequest event */
    const handler = (e) => {
      const xhr = /** @type {XMLHttpRequest | null} */ (e.detail.xhr);
      if (!xhr) return;

      const header = xhr.getResponseHeader('X-Crap-Toast');
      if (header) {
        const isError = xhr.status >= 400;
        const fallbackType = isError ? 'error' : 'success';
        try {
          /** @type {{ message: string, type?: string }} */
          const data = JSON.parse(header);
          this.show(data.message, /** @type {any} */ (data.type || fallbackType));
        } catch {
          this.show(header, fallbackType);
        }
      }
    };

    document.body.addEventListener('htmx:afterRequest', handler);
  }
}

customElements.define('crap-toast', CrapToast);

/**
 * Global toast API.
 * Finds or creates the <crap-toast> element and delegates to its show() method.
 * @namespace
 */
window.CrapToast = {
  /**
   * @param {string} message
   * @param {'success' | 'error' | 'info'} [type='info']
   * @param {number} [duration=3000]
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
