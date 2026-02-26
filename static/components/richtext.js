/**
 * <crap-richtext> — ProseMirror-based WYSIWYG editor.
 *
 * Wraps a hidden <textarea> with a rich editor. The textarea remains
 * the form submission source — the editor syncs HTML back on every change.
 *
 * Requires `window.ProseMirror` (loaded via prosemirror.js IIFE bundle).
 * Falls back to showing the plain textarea if ProseMirror is unavailable.
 *
 * @example
 * <crap-richtext>
 *   <textarea name="content" style="display:none">...</textarea>
 * </crap-richtext>
 */
class CrapRichtext extends HTMLElement {
  constructor() {
    super();

    /** @type {import('prosemirror-view').EditorView | null} */
    this._view = null;

    this.attachShadow({ mode: 'open' });
  }

  connectedCallback() {
    const PM = /** @type {any} */ (window).ProseMirror;
    /** @type {HTMLTextAreaElement | null} */
    const textarea = this.querySelector('textarea');

    // Graceful degradation: no ProseMirror -> show plain textarea
    if (!PM || !textarea) {
      if (textarea) textarea.style.display = '';
      return;
    }

    textarea.style.display = 'none';

    // Build schema with list support
    const schema = new PM.Schema({
      nodes: PM.addListNodes(
        PM.basicSchema.spec.nodes,
        'paragraph block*',
        'block'
      ),
      marks: PM.basicSchema.spec.marks,
    });

    // Parse existing HTML content into a ProseMirror document
    const container = document.createElement('div');
    container.innerHTML = textarea.value || '';
    const doc = PM.DOMParser.fromSchema(schema).parse(container);

    const isReadonly = textarea.hasAttribute('readonly');

    // Input rules: smart quotes, em dash, ellipsis, plus block-level rules
    const rules = [
      ...PM.smartQuotes,
      PM.emDash,
      PM.ellipsis,
      // > blockquote
      PM.wrappingInputRule(/^\s*>\s$/, schema.nodes.blockquote),
      // 1. ordered list
      PM.wrappingInputRule(
        /^(\d+)\.\s$/,
        schema.nodes.ordered_list,
        (match) => ({ order: +match[1] }),
        (match, node) => node.childCount + node.attrs.order === +match[1]
      ),
      // - or * bullet list
      PM.wrappingInputRule(/^\s*([-*])\s$/, schema.nodes.bullet_list),
      // ``` code block
      PM.textblockTypeInputRule(/^```$/, schema.nodes.code_block),
      // # ## ### headings
      PM.textblockTypeInputRule(
        /^(#{1,3})\s$/,
        schema.nodes.heading,
        (match) => ({ level: match[1].length })
      ),
    ];

    // Keymap for list operations
    const listKeymap = {};
    if (schema.nodes.list_item) {
      listKeymap['Enter'] = PM.splitListItem(schema.nodes.list_item);
      listKeymap['Tab'] = PM.sinkListItem(schema.nodes.list_item);
      listKeymap['Shift-Tab'] = PM.liftListItem(schema.nodes.list_item);
    }

    // Plugin to track active marks/nodes for toolbar state
    const toolbarPluginKey = new PM.PluginKey('toolbar');
    const toolbarPlugin = new PM.Plugin({
      key: toolbarPluginKey,
      view: () => ({
        update: (/** @type {any} */ view) => {
          this._updateToolbar(view.state, schema);
        },
      }),
    });

    const plugins = [
      PM.inputRules({ rules }),
      PM.keymap(listKeymap),
      PM.keymap({
        'Mod-z': PM.undo,
        'Mod-shift-z': PM.redo,
        'Mod-y': PM.redo,
        'Mod-b': PM.toggleMark(schema.marks.strong),
        'Mod-i': PM.toggleMark(schema.marks.em),
        'Mod-`': PM.toggleMark(schema.marks.code),
      }),
      PM.keymap(PM.baseKeymap),
      PM.dropCursor(),
      PM.gapCursor(),
      PM.history(),
      toolbarPlugin,
    ];

    const state = PM.EditorState.create({ doc, plugins });

    // Render Shadow DOM
    this.shadowRoot.innerHTML = `
      <style>${CrapRichtext._styles()}</style>
      <div class="richtext">
        ${isReadonly ? '' : `<div class="richtext__toolbar">${CrapRichtext._toolbarHTML()}</div>`}
        <div class="richtext__editor"></div>
      </div>
    `;

    const editorEl = this.shadowRoot.querySelector('.richtext__editor');

    this._view = new PM.EditorView(editorEl, {
      state,
      editable: () => !isReadonly,
      dispatchTransaction: (/** @type {any} */ tr) => {
        const newState = this._view.state.apply(tr);
        this._view.updateState(newState);
        if (tr.docChanged) {
          const fragment = PM.DOMSerializer
            .fromSchema(schema)
            .serializeFragment(newState.doc.content);
          const div = document.createElement('div');
          div.appendChild(fragment);
          textarea.value = div.innerHTML;
        }
      },
    });

    // Wire up toolbar buttons
    if (!isReadonly) {
      this._bindToolbar(schema);
    }

    // Initial toolbar state
    this._updateToolbar(state, schema);
  }

  disconnectedCallback() {
    if (this._view) {
      this._view.destroy();
      this._view = null;
    }
  }

  /**
   * Bind click handlers to all toolbar buttons.
   * @param {any} schema - ProseMirror schema
   */
  _bindToolbar(schema) {
    const PM = /** @type {any} */ (window).ProseMirror;
    const toolbar = this.shadowRoot.querySelector('.richtext__toolbar');
    if (!toolbar) return;

    /** @type {Record<string, () => void>} */
    const commands = {
      bold: () => PM.toggleMark(schema.marks.strong)(this._view.state, this._view.dispatch),
      italic: () => PM.toggleMark(schema.marks.em)(this._view.state, this._view.dispatch),
      code: () => PM.toggleMark(schema.marks.code)(this._view.state, this._view.dispatch),
      link: () => {
        const { state, dispatch } = this._view;
        if (this._markActive(state, schema.marks.link)) {
          PM.toggleMark(schema.marks.link)(state, dispatch);
        } else {
          const href = prompt('Link URL:');
          if (href) {
            PM.toggleMark(schema.marks.link, { href })(state, dispatch);
          }
        }
      },
      h1: () => PM.setBlockType(schema.nodes.heading, { level: 1 })(this._view.state, this._view.dispatch),
      h2: () => PM.setBlockType(schema.nodes.heading, { level: 2 })(this._view.state, this._view.dispatch),
      h3: () => PM.setBlockType(schema.nodes.heading, { level: 3 })(this._view.state, this._view.dispatch),
      paragraph: () => PM.setBlockType(schema.nodes.paragraph)(this._view.state, this._view.dispatch),
      ul: () => PM.wrapInList(schema.nodes.bullet_list)(this._view.state, this._view.dispatch),
      ol: () => PM.wrapInList(schema.nodes.ordered_list)(this._view.state, this._view.dispatch),
      blockquote: () => PM.wrapIn(schema.nodes.blockquote)(this._view.state, this._view.dispatch),
      hr: () => {
        const { state, dispatch } = this._view;
        dispatch(state.tr.replaceSelectionWith(schema.nodes.horizontal_rule.create()));
      },
      undo: () => PM.undo(this._view.state, this._view.dispatch),
      redo: () => PM.redo(this._view.state, this._view.dispatch),
    };

    toolbar.addEventListener('click', (e) => {
      const btn = /** @type {HTMLElement} */ (e.target).closest('button[data-cmd]');
      if (!btn) return;
      const cmd = btn.getAttribute('data-cmd');
      if (cmd && commands[cmd]) {
        commands[cmd]();
        this._view.focus();
      }
    });
  }

  /**
   * Check if a mark is active in the current selection.
   * @param {any} state
   * @param {any} markType
   * @returns {boolean}
   */
  _markActive(state, markType) {
    const { from, $from, to, empty } = state.selection;
    if (empty) return !!markType.isInSet(state.storedMarks || $from.marks());
    return state.doc.rangeHasMark(from, to, markType);
  }

  /**
   * Update toolbar button active states based on current editor state.
   * @param {any} state
   * @param {any} schema
   */
  _updateToolbar(state, schema) {
    const toolbar = this.shadowRoot?.querySelector('.richtext__toolbar');
    if (!toolbar) return;

    /** @type {NodeListOf<HTMLButtonElement>} */
    const buttons = toolbar.querySelectorAll('button[data-cmd]');

    buttons.forEach((btn) => {
      const cmd = btn.getAttribute('data-cmd');
      let active = false;

      switch (cmd) {
        case 'bold':
          active = this._markActive(state, schema.marks.strong);
          break;
        case 'italic':
          active = this._markActive(state, schema.marks.em);
          break;
        case 'code':
          active = this._markActive(state, schema.marks.code);
          break;
        case 'link':
          active = this._markActive(state, schema.marks.link);
          break;
        case 'h1':
        case 'h2':
        case 'h3': {
          const level = parseInt(cmd[1]);
          const { $from } = state.selection;
          active = $from.parent.type === schema.nodes.heading && $from.parent.attrs.level === level;
          break;
        }
        case 'paragraph': {
          const { $from } = state.selection;
          active = $from.parent.type === schema.nodes.paragraph;
          break;
        }
      }

      btn.classList.toggle('active', active);
    });
  }

  /**
   * Generate toolbar button HTML.
   * @returns {string}
   */
  static _toolbarHTML() {
    return `
      <div class="richtext__toolbar-group">
        <button type="button" data-cmd="bold" title="Bold (Ctrl+B)"><strong>B</strong></button>
        <button type="button" data-cmd="italic" title="Italic (Ctrl+I)"><em>I</em></button>
        <button type="button" data-cmd="code" title="Inline code (Ctrl+\`)"><code>&lt;/&gt;</code></button>
        <button type="button" data-cmd="link" title="Link">Link</button>
      </div>
      <div class="richtext__toolbar-group">
        <button type="button" data-cmd="h1" title="Heading 1">H1</button>
        <button type="button" data-cmd="h2" title="Heading 2">H2</button>
        <button type="button" data-cmd="h3" title="Heading 3">H3</button>
        <button type="button" data-cmd="paragraph" title="Paragraph">P</button>
      </div>
      <div class="richtext__toolbar-group">
        <button type="button" data-cmd="ul" title="Bullet list">UL</button>
        <button type="button" data-cmd="ol" title="Ordered list">OL</button>
        <button type="button" data-cmd="blockquote" title="Blockquote">Quote</button>
        <button type="button" data-cmd="hr" title="Horizontal rule">HR</button>
      </div>
      <div class="richtext__toolbar-group">
        <button type="button" data-cmd="undo" title="Undo (Ctrl+Z)">Undo</button>
        <button type="button" data-cmd="redo" title="Redo (Ctrl+Shift+Z)">Redo</button>
      </div>
    `;
  }

  /**
   * Shadow DOM styles. Uses CSS custom properties from :root (penetrate shadow boundary).
   * @returns {string}
   */
  static _styles() {
    return `
      :host {
        display: block;
      }

      .richtext {
        border: 1px solid var(--input-border, #e0e0e0);
        border-radius: var(--radius-md, 6px);
        background: var(--input-bg, #fff);
        box-shadow: var(--shadow-sm, 0 1px 2px rgba(0,0,0,0.04));
        overflow: hidden;
      }

      .richtext:focus-within {
        border-color: var(--color-primary, #1677ff);
        box-shadow: 0 0 0 2px var(--color-primary-bg, rgba(22, 119, 255, 0.06));
      }

      /* -- Toolbar -- */

      .richtext__toolbar {
        display: flex;
        flex-wrap: wrap;
        gap: 2px;
        padding: 6px 8px;
        border-bottom: 1px solid var(--border-color, #e0e0e0);
      }

      .richtext__toolbar-group {
        display: flex;
        gap: 2px;
      }

      .richtext__toolbar-group:not(:last-child)::after {
        content: '';
        width: 1px;
        margin: 2px 4px;
        background: var(--border-color, #e0e0e0);
      }

      .richtext__toolbar button {
        all: unset;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        min-width: 28px;
        height: 28px;
        padding: 0 6px;
        border-radius: var(--radius-sm, 4px);
        font-family: inherit;
        font-size: 12px;
        font-weight: 500;
        color: var(--text-secondary, rgba(0, 0, 0, 0.65));
        cursor: pointer;
        box-sizing: border-box;
      }

      .richtext__toolbar button:hover {
        background: var(--bg-hover, rgba(0, 0, 0, 0.04));
        color: var(--text-primary, rgba(0, 0, 0, 0.88));
      }

      .richtext__toolbar button.active {
        background: var(--color-primary-bg, rgba(22, 119, 255, 0.06));
        color: var(--color-primary, #1677ff);
      }

      .richtext__toolbar button code {
        font-family: monospace;
        font-size: 12px;
      }

      /* -- Editor area -- */

      .richtext__editor {
        min-height: 200px;
        max-height: 600px;
        overflow-y: auto;
      }

      .richtext__editor .ProseMirror {
        padding: 12px 16px;
        min-height: 200px;
        outline: none;
        font-family: inherit;
        font-size: var(--text-base, 1rem);
        line-height: 1.6;
        color: var(--text-primary, rgba(0, 0, 0, 0.88));
      }

      /* ProseMirror content styles */

      .richtext__editor .ProseMirror p {
        margin: 0 0 0.75em;
      }

      .richtext__editor .ProseMirror p:last-child {
        margin-bottom: 0;
      }

      .richtext__editor .ProseMirror h1,
      .richtext__editor .ProseMirror h2,
      .richtext__editor .ProseMirror h3 {
        margin: 1em 0 0.5em;
        font-weight: 600;
        line-height: 1.3;
      }

      .richtext__editor .ProseMirror h1:first-child,
      .richtext__editor .ProseMirror h2:first-child,
      .richtext__editor .ProseMirror h3:first-child {
        margin-top: 0;
      }

      .richtext__editor .ProseMirror h1 { font-size: 1.5em; }
      .richtext__editor .ProseMirror h2 { font-size: 1.25em; }
      .richtext__editor .ProseMirror h3 { font-size: 1.1em; }

      .richtext__editor .ProseMirror strong { font-weight: 600; }

      .richtext__editor .ProseMirror code {
        background: var(--bg-hover, rgba(0, 0, 0, 0.06));
        padding: 0.15em 0.35em;
        border-radius: 3px;
        font-family: monospace;
        font-size: 0.9em;
      }

      .richtext__editor .ProseMirror pre {
        background: var(--bg-hover, rgba(0, 0, 0, 0.04));
        border-radius: var(--radius-sm, 4px);
        padding: 12px 16px;
        margin: 0.75em 0;
        overflow-x: auto;
      }

      .richtext__editor .ProseMirror pre code {
        background: none;
        padding: 0;
        border-radius: 0;
      }

      .richtext__editor .ProseMirror blockquote {
        border-left: 3px solid var(--border-color-hover, #d9d9d9);
        margin: 0.75em 0;
        padding-left: 1em;
        color: var(--text-secondary, rgba(0, 0, 0, 0.65));
      }

      .richtext__editor .ProseMirror ul,
      .richtext__editor .ProseMirror ol {
        margin: 0.75em 0;
        padding-left: 1.5em;
      }

      .richtext__editor .ProseMirror li {
        margin-bottom: 0.25em;
      }

      .richtext__editor .ProseMirror li p {
        margin: 0;
      }

      .richtext__editor .ProseMirror hr {
        border: none;
        border-top: 1px solid var(--border-color, #e0e0e0);
        margin: 1em 0;
      }

      .richtext__editor .ProseMirror a {
        color: var(--color-primary, #1677ff);
        text-decoration: underline;
      }

      .richtext__editor .ProseMirror img {
        max-width: 100%;
      }

      /* ProseMirror plugin styles */

      .ProseMirror-gapcursor {
        display: none;
        pointer-events: none;
        position: absolute;
      }

      .ProseMirror-gapcursor:after {
        content: '';
        display: block;
        position: absolute;
        top: -2px;
        width: 20px;
        border-top: 1px solid var(--text-primary, black);
        animation: ProseMirror-cursor-blink 1.1s steps(2, start) infinite;
      }

      @keyframes ProseMirror-cursor-blink {
        to { visibility: hidden; }
      }

      .ProseMirror-focused .ProseMirror-gapcursor {
        display: block;
      }

      .ProseMirror .ProseMirror-selectednode {
        outline: 2px solid var(--color-primary, #1677ff);
      }
    `;
  }
}

customElements.define('crap-richtext', CrapRichtext);
