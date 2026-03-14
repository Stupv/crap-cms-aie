/**
 * <crap-richtext> — ProseMirror-based WYSIWYG editor.
 *
 * Wraps a hidden <textarea> with a rich editor. The textarea remains
 * the form submission source — the editor syncs HTML back on every change.
 *
 * Requires `window.ProseMirror` (loaded via prosemirror.js IIFE bundle).
 * Falls back to showing the plain textarea if ProseMirror is unavailable.
 *
 * Supports `data-features` attribute (JSON array) to limit which toolbar
 * buttons and editing features are available. When absent, all features
 * are enabled.
 *
 * Available features: "bold", "italic", "code", "link", "heading",
 * "blockquote", "orderedList", "bulletList", "codeBlock", "horizontalRule"
 *
 * Supports `data-nodes` attribute (JSON array of custom node definitions)
 * for embedding structured components (CTAs, embeds, etc.) in the editor.
 *
 * @example
 * <crap-richtext>
 *   <textarea name="content" style="display:none">...</textarea>
 * </crap-richtext>
 *
 * <crap-richtext data-features='["bold","italic","heading","link"]'>
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

    // Parse enabled features (empty = all enabled)
    /** @type {Set<string>|null} */
    let enabledFeatures = null;
    const featuresAttr = this.getAttribute('data-features');
    if (featuresAttr) {
      try {
        const arr = JSON.parse(featuresAttr);
        if (Array.isArray(arr) && arr.length > 0) {
          enabledFeatures = new Set(arr);
        }
      } catch { /* ignore, all features enabled */ }
    }

    /**
     * Check if a feature is enabled.
     * @param {string} name
     * @returns {boolean}
     */
    const has = (name) => enabledFeatures === null || enabledFeatures.has(name);

    // Build schema — conditionally include nodes and marks
    const baseNodes = PM.basicSchema.spec.nodes;
    let nodes = baseNodes;

    // Add list nodes only if list features are enabled
    if (has('orderedList') || has('bulletList')) {
      nodes = PM.addListNodes(nodes, 'paragraph block*', 'block');
    }

    // Remove nodes based on features
    if (!has('heading')) {
      nodes = nodes.remove('heading');
    }
    if (!has('codeBlock')) {
      nodes = nodes.remove('code_block');
    }
    if (!has('blockquote')) {
      nodes = nodes.remove('blockquote');
    }
    if (!has('horizontalRule')) {
      nodes = nodes.remove('horizontal_rule');
    }

    // Build marks — conditionally include
    let marksObj = {};
    const baseMarks = PM.basicSchema.spec.marks;
    if (has('bold') && baseMarks.get('strong')) {
      marksObj.strong = baseMarks.get('strong');
    }
    if (has('italic') && baseMarks.get('em')) {
      marksObj.em = baseMarks.get('em');
    }
    if (has('code') && baseMarks.get('code')) {
      marksObj.code = baseMarks.get('code');
    }
    if (has('link') && baseMarks.get('link')) {
      marksObj.link = baseMarks.get('link');
    }

    // Parse custom nodes from data-nodes attribute
    /** @type {Array<{name: string, label: string, inline: boolean, attrs: Array<{name: string, type: string, label: string, required: boolean, default?: any, options?: Array<{label: string, value: string}>}>}>} */
    const customNodes = [];
    const nodesAttr = this.getAttribute('data-nodes');
    if (nodesAttr) {
      try {
        const parsed = JSON.parse(nodesAttr);
        if (Array.isArray(parsed)) customNodes.push(...parsed);
      } catch { /* ignore */ }
    }

    // Inject custom NodeSpecs into schema
    for (const nodeDef of customNodes) {
      nodes = nodes.addToEnd(nodeDef.name, {
        group: nodeDef.inline ? 'inline' : 'block',
        inline: nodeDef.inline,
        atom: true,
        attrs: Object.fromEntries(
          (nodeDef.attrs || []).map(a => [a.name, { default: a.default ?? '' }])
        ),
        toDOM(node) {
          return ['crap-node', {
            'data-type': nodeDef.name,
            'data-attrs': JSON.stringify(node.attrs),
          }];
        },
        parseDOM: [{
          tag: `crap-node[data-type="${nodeDef.name}"]`,
          getAttrs(dom) {
            try { return JSON.parse(dom.getAttribute('data-attrs') || '{}'); }
            catch { return {}; }
          },
        }],
      });
    }

    const schema = new PM.Schema({
      nodes,
      marks: marksObj,
    });

    // Read storage format: "html" (default) or "json" (ProseMirror JSON)
    const format = this.getAttribute('data-format') || 'html';

    // Parse existing content into a ProseMirror document
    let doc;
    if (format === 'json' && textarea.value.trim()) {
      try {
        const parsed = JSON.parse(textarea.value);
        doc = PM.Node.fromJSON(schema, parsed);
      } catch {
        // Fallback to empty doc on parse error
        doc = schema.topNodeType.createAndFill();
      }
    } else {
      const container = document.createElement('div');
      container.innerHTML = textarea.value || '';
      doc = PM.DOMParser.fromSchema(schema).parse(container);
    }

    const isReadonly = textarea.hasAttribute('readonly');

    // Input rules: smart quotes, em dash, ellipsis, plus conditional block-level rules
    const rules = [
      ...PM.smartQuotes,
      PM.emDash,
      PM.ellipsis,
    ];

    if (has('blockquote') && schema.nodes.blockquote) {
      rules.push(PM.wrappingInputRule(/^\s*>\s$/, schema.nodes.blockquote));
    }
    if (has('orderedList') && schema.nodes.ordered_list) {
      rules.push(PM.wrappingInputRule(
        /^(\d+)\.\s$/,
        schema.nodes.ordered_list,
        (match) => ({ order: +match[1] }),
        (match, node) => node.childCount + node.attrs.order === +match[1]
      ));
    }
    if (has('bulletList') && schema.nodes.bullet_list) {
      rules.push(PM.wrappingInputRule(/^\s*([-*])\s$/, schema.nodes.bullet_list));
    }
    if (has('codeBlock') && schema.nodes.code_block) {
      rules.push(PM.textblockTypeInputRule(/^```$/, schema.nodes.code_block));
    }
    if (has('heading') && schema.nodes.heading) {
      rules.push(PM.textblockTypeInputRule(
        /^(#{1,3})\s$/,
        schema.nodes.heading,
        (match) => ({ level: match[1].length })
      ));
    }

    // Keymap for list operations
    const listKeymap = {};
    if (schema.nodes.list_item) {
      listKeymap['Enter'] = PM.splitListItem(schema.nodes.list_item);
      listKeymap['Tab'] = PM.sinkListItem(schema.nodes.list_item);
      listKeymap['Shift-Tab'] = PM.liftListItem(schema.nodes.list_item);
    }

    // Keyboard shortcuts — only for enabled marks
    const markKeymap = {};
    markKeymap['Mod-z'] = PM.undo;
    markKeymap['Mod-shift-z'] = PM.redo;
    markKeymap['Mod-y'] = PM.redo;
    if (has('bold') && schema.marks.strong) {
      markKeymap['Mod-b'] = PM.toggleMark(schema.marks.strong);
    }
    if (has('italic') && schema.marks.em) {
      markKeymap['Mod-i'] = PM.toggleMark(schema.marks.em);
    }
    if (has('code') && schema.marks.code) {
      markKeymap['Mod-`'] = PM.toggleMark(schema.marks.code);
    }

    // Plugin to track active marks/nodes for toolbar state
    const toolbarPluginKey = new PM.PluginKey('toolbar');
    const toolbarPlugin = new PM.Plugin({
      key: toolbarPluginKey,
      view: () => ({
        update: (/** @type {any} */ view) => {
          this._updateToolbar(view.state, schema, has);
        },
      }),
    });

    const plugins = [
      PM.inputRules({ rules }),
      PM.keymap(listKeymap),
      PM.keymap(markKeymap),
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
      <div class="richtext${this.hasAttribute('data-no-resize') ? ' richtext--no-resize' : ''}">
        ${isReadonly ? '' : `<div class="richtext__toolbar">${CrapRichtext._toolbarHTML(has, customNodes)}</div>`}
        <div class="richtext__editor"></div>
      </div>
    `;

    const editorEl = this.shadowRoot.querySelector('.richtext__editor');

    // Store custom node defs on instance for toolbar/modal use
    /** @type {typeof customNodes} */
    this._customNodes = customNodes;

    this._view = new PM.EditorView(editorEl, {
      state,
      editable: () => !isReadonly,
      nodeViews: Object.fromEntries(
        customNodes.map(nd => [nd.name, (node, view, getPos) =>
          new CustomNodeView(node, view, getPos, nd)
        ])
      ),
      dispatchTransaction: (/** @type {any} */ tr) => {
        const newState = this._view.state.apply(tr);
        this._view.updateState(newState);
        if (tr.docChanged) {
          if (format === 'json') {
            textarea.value = JSON.stringify(newState.doc.toJSON());
          } else {
            const fragment = PM.DOMSerializer
              .fromSchema(schema)
              .serializeFragment(newState.doc.content);
            const div = document.createElement('div');
            div.appendChild(fragment);
            textarea.value = div.innerHTML;
          }
        }
      },
    });

    // Wire up toolbar buttons
    if (!isReadonly) {
      this._bindToolbar(schema, has);
    }

    // Initial toolbar state
    this._updateToolbar(state, schema, has);
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
   * @param {(name: string) => boolean} has - feature check
   */
  _bindToolbar(schema, has) {
    const PM = /** @type {any} */ (window).ProseMirror;
    const toolbar = this.shadowRoot.querySelector('.richtext__toolbar');
    if (!toolbar) return;

    /** @type {Record<string, () => void>} */
    const commands = {};

    if (has('bold') && schema.marks.strong) {
      commands.bold = () => PM.toggleMark(schema.marks.strong)(this._view.state, this._view.dispatch);
    }
    if (has('italic') && schema.marks.em) {
      commands.italic = () => PM.toggleMark(schema.marks.em)(this._view.state, this._view.dispatch);
    }
    if (has('code') && schema.marks.code) {
      commands.code = () => PM.toggleMark(schema.marks.code)(this._view.state, this._view.dispatch);
    }
    if (has('link') && schema.marks.link) {
      commands.link = () => {
        const { state, dispatch } = this._view;
        if (this._markActive(state, schema.marks.link)) {
          PM.toggleMark(schema.marks.link)(state, dispatch);
        } else {
          const href = prompt('Link URL:');
          if (href) {
            PM.toggleMark(schema.marks.link, { href })(state, dispatch);
          }
        }
      };
    }
    if (has('heading') && schema.nodes.heading) {
      commands.h1 = () => PM.setBlockType(schema.nodes.heading, { level: 1 })(this._view.state, this._view.dispatch);
      commands.h2 = () => PM.setBlockType(schema.nodes.heading, { level: 2 })(this._view.state, this._view.dispatch);
      commands.h3 = () => PM.setBlockType(schema.nodes.heading, { level: 3 })(this._view.state, this._view.dispatch);
      commands.paragraph = () => PM.setBlockType(schema.nodes.paragraph)(this._view.state, this._view.dispatch);
    }
    if (has('bulletList') && schema.nodes.bullet_list) {
      commands.ul = () => PM.wrapInList(schema.nodes.bullet_list)(this._view.state, this._view.dispatch);
    }
    if (has('orderedList') && schema.nodes.ordered_list) {
      commands.ol = () => PM.wrapInList(schema.nodes.ordered_list)(this._view.state, this._view.dispatch);
    }
    if (has('blockquote') && schema.nodes.blockquote) {
      commands.blockquote = () => PM.wrapIn(schema.nodes.blockquote)(this._view.state, this._view.dispatch);
    }
    if (has('horizontalRule') && schema.nodes.horizontal_rule) {
      commands.hr = () => {
        const { state, dispatch } = this._view;
        dispatch(state.tr.replaceSelectionWith(schema.nodes.horizontal_rule.create()));
      };
    }
    commands.undo = () => PM.undo(this._view.state, this._view.dispatch);
    commands.redo = () => PM.redo(this._view.state, this._view.dispatch);

    // Custom node insert commands
    for (const nd of (this._customNodes || [])) {
      commands[`insert-${nd.name}`] = () => {
        const nodeType = schema.nodes[nd.name];
        if (!nodeType) return;
        const defaultAttrs = Object.fromEntries(
          (nd.attrs || []).map(a => [a.name, a.default ?? ''])
        );
        const { state, dispatch } = this._view;
        const node = nodeType.create(defaultAttrs);
        const tr = state.tr.replaceSelectionWith(node);
        dispatch(tr);
        // Open edit modal for the inserted node
        const pos = tr.mapping.map(state.selection.from);
        this._openNodeEditModal(nd, defaultAttrs, pos - 1);
      };
    }

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
   * @param {(name: string) => boolean} has - feature check
   */
  _updateToolbar(state, schema, has) {
    const toolbar = this.shadowRoot?.querySelector('.richtext__toolbar');
    if (!toolbar) return;

    /** @type {NodeListOf<HTMLButtonElement>} */
    const buttons = toolbar.querySelectorAll('button[data-cmd]');

    buttons.forEach((btn) => {
      const cmd = btn.getAttribute('data-cmd');
      let active = false;

      switch (cmd) {
        case 'bold':
          active = has('bold') && schema.marks.strong && this._markActive(state, schema.marks.strong);
          break;
        case 'italic':
          active = has('italic') && schema.marks.em && this._markActive(state, schema.marks.em);
          break;
        case 'code':
          active = has('code') && schema.marks.code && this._markActive(state, schema.marks.code);
          break;
        case 'link':
          active = has('link') && schema.marks.link && this._markActive(state, schema.marks.link);
          break;
        case 'h1':
        case 'h2':
        case 'h3': {
          if (has('heading') && schema.nodes.heading) {
            const level = parseInt(cmd[1]);
            const { $from } = state.selection;
            active = $from.parent.type === schema.nodes.heading && $from.parent.attrs.level === level;
          }
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
   * Open the edit modal for a custom node at the given position.
   * @param {object} nodeDef - custom node definition
   * @param {object} attrs - current attribute values
   * @param {number} pos - node position in the document
   */
  _openNodeEditModal(nodeDef, attrs, pos) {
    // Remove any existing modal
    const existing = this.shadowRoot.querySelector('.crap-node-modal');
    if (existing) existing.remove();

    const modal = document.createElement('div');
    modal.className = 'crap-node-modal';

    const formFields = (nodeDef.attrs || []).map(a => {
      const val = attrs[a.name] ?? a.default ?? '';
      let input;
      switch (a.type) {
        case 'textarea':
          input = `<textarea class="crap-node-modal__input" data-attr="${a.name}" rows="3"${a.required ? ' required' : ''}>${val}</textarea>`;
          break;
        case 'checkbox':
          input = `<label class="crap-node-modal__checkbox"><input type="checkbox" data-attr="${a.name}"${val ? ' checked' : ''}> ${a.label}</label>`;
          break;
        case 'select':
          input = `<select class="crap-node-modal__input" data-attr="${a.name}"${a.required ? ' required' : ''}>
            ${(a.options || []).map(o => `<option value="${o.value}"${o.value === val ? ' selected' : ''}>${o.label}</option>`).join('')}
          </select>`;
          break;
        case 'number':
          input = `<input type="number" class="crap-node-modal__input" data-attr="${a.name}" value="${val}"${a.required ? ' required' : ''}>`;
          break;
        default:
          input = `<input type="text" class="crap-node-modal__input" data-attr="${a.name}" value="${val}"${a.required ? ' required' : ''}>`;
      }
      if (a.type === 'checkbox') return `<div class="crap-node-modal__field">${input}</div>`;
      return `<div class="crap-node-modal__field"><label class="crap-node-modal__label">${a.label}${a.required ? ' *' : ''}</label>${input}</div>`;
    }).join('');

    modal.innerHTML = `
      <div class="crap-node-modal__backdrop"></div>
      <div class="crap-node-modal__dialog">
        <div class="crap-node-modal__header">${nodeDef.label}</div>
        <div class="crap-node-modal__body">${formFields}</div>
        <div class="crap-node-modal__footer">
          <button type="button" class="crap-node-modal__btn crap-node-modal__btn--cancel">Cancel</button>
          <button type="button" class="crap-node-modal__btn crap-node-modal__btn--ok">OK</button>
        </div>
      </div>
    `;

    this.shadowRoot.appendChild(modal);

    // Focus first input
    const firstInput = modal.querySelector('input, textarea, select');
    if (firstInput) firstInput.focus();

    const close = () => modal.remove();

    modal.querySelector('.crap-node-modal__backdrop').addEventListener('click', close);
    modal.querySelector('.crap-node-modal__btn--cancel').addEventListener('click', close);
    modal.querySelector('.crap-node-modal__btn--ok').addEventListener('click', () => {
      const newAttrs = {};
      for (const a of (nodeDef.attrs || [])) {
        const el = modal.querySelector(`[data-attr="${a.name}"]`);
        if (!el) continue;
        if (a.type === 'checkbox') {
          newAttrs[a.name] = el.checked;
        } else {
          newAttrs[a.name] = el.value;
        }
      }
      // Update the node at pos
      const { state, dispatch } = this._view;
      try {
        const node = state.doc.nodeAt(pos);
        if (node) {
          const tr = state.tr.setNodeMarkup(pos, null, newAttrs);
          dispatch(tr);
        }
      } catch { /* position might have changed */ }
      close();
      this._view.focus();
    });
  }

  /**
   * Generate toolbar button HTML based on enabled features.
   * @param {(name: string) => boolean} has - feature check
   * @param {Array<{name: string, label: string}>} [customNodes] - custom node defs
   * @returns {string}
   */
  static _toolbarHTML(has, customNodes) {
    let html = '';

    // Inline marks group
    const inlineButtons = [];
    if (has('bold')) inlineButtons.push('<button type="button" data-cmd="bold" title="Bold (Ctrl+B)"><strong>B</strong></button>');
    if (has('italic')) inlineButtons.push('<button type="button" data-cmd="italic" title="Italic (Ctrl+I)"><em>I</em></button>');
    if (has('code')) inlineButtons.push('<button type="button" data-cmd="code" title="Inline code (Ctrl+`)"><code>&lt;/&gt;</code></button>');
    if (has('link')) inlineButtons.push('<button type="button" data-cmd="link" title="Link">Link</button>');
    if (inlineButtons.length > 0) {
      html += `<div class="richtext__toolbar-group">${inlineButtons.join('')}</div>`;
    }

    // Block type group
    const blockButtons = [];
    if (has('heading')) {
      blockButtons.push('<button type="button" data-cmd="h1" title="Heading 1">H1</button>');
      blockButtons.push('<button type="button" data-cmd="h2" title="Heading 2">H2</button>');
      blockButtons.push('<button type="button" data-cmd="h3" title="Heading 3">H3</button>');
      blockButtons.push('<button type="button" data-cmd="paragraph" title="Paragraph">P</button>');
    }
    if (blockButtons.length > 0) {
      html += `<div class="richtext__toolbar-group">${blockButtons.join('')}</div>`;
    }

    // List/block group
    const listButtons = [];
    if (has('bulletList')) listButtons.push('<button type="button" data-cmd="ul" title="Bullet list">UL</button>');
    if (has('orderedList')) listButtons.push('<button type="button" data-cmd="ol" title="Ordered list">OL</button>');
    if (has('blockquote')) listButtons.push('<button type="button" data-cmd="blockquote" title="Blockquote">Quote</button>');
    if (has('horizontalRule')) listButtons.push('<button type="button" data-cmd="hr" title="Horizontal rule">HR</button>');
    if (listButtons.length > 0) {
      html += `<div class="richtext__toolbar-group">${listButtons.join('')}</div>`;
    }

    // Custom node insert buttons
    if (customNodes && customNodes.length > 0) {
      const insertButtons = customNodes.map(nd =>
        `<button type="button" data-cmd="insert-${nd.name}" title="Insert ${nd.label}">${nd.label}</button>`
      ).join('');
      html += `<div class="richtext__toolbar-group">${insertButtons}</div>`;
    }

    // Undo/redo always present
    html += `<div class="richtext__toolbar-group">
      <button type="button" data-cmd="undo" title="Undo (Ctrl+Z)">Undo</button>
      <button type="button" data-cmd="redo" title="Redo (Ctrl+Shift+Z)">Redo</button>
    </div>`;

    return html;
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
        display: flex;
        flex-direction: column;
        resize: vertical;
      }

      .richtext--no-resize {
        resize: none;
        max-height: 600px;
      }

      .richtext:focus-within {
        border-color: var(--color-primary, #1677ff);
        box-shadow: 0 0 0 2px var(--color-primary-bg, rgba(22, 119, 255, 0.06));
      }

      /* -- Toolbar -- */

      .richtext__toolbar {
        display: flex;
        flex-wrap: wrap;
        gap: var(--space-2xs, 2px);
        padding: 6px var(--space-sm, 8px);
        border-bottom: 1px solid var(--border-color, #e0e0e0);
      }

      .richtext__toolbar-group {
        display: flex;
        gap: var(--space-2xs, 2px);
      }

      .richtext__toolbar-group:not(:last-child)::after {
        content: '';
        width: 1px;
        margin: var(--space-2xs, 2px) var(--space-xs, 4px);
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
        font-size: var(--text-xs, 0.75rem);
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
        font-size: var(--text-xs, 0.75rem);
      }

      /* -- Editor area -- */

      .richtext__editor {
        min-height: 200px;
        overflow-y: auto;
        flex: 1;
      }

      .richtext__editor .ProseMirror {
        padding: var(--space-md, 0.75rem) var(--space-lg, 1rem);
        min-height: 200px;
        outline: none;
        font-family: inherit;
        font-size: var(--text-base, 0.875rem);
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
        padding: var(--space-md, 0.75rem) var(--space-lg, 1rem);
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

      /* -- Custom node cards/pills -- */

      .crap-custom-node {
        display: flex;
        align-items: center;
        gap: var(--space-sm, 0.5rem);
        padding: var(--space-sm, 0.5rem) var(--space-md, 0.75rem);
        margin: var(--space-xs, 0.25rem) 0;
        border: 1px solid var(--border-color, #e0e0e0);
        border-radius: var(--radius-sm, 4px);
        background: var(--bg-hover, rgba(0, 0, 0, 0.02));
        cursor: pointer;
        user-select: none;
      }

      .crap-custom-node--inline {
        display: inline-flex;
        margin: 0 var(--space-2xs, 2px);
        padding: var(--space-2xs, 2px) var(--space-sm, 0.5rem);
        vertical-align: middle;
        border-radius: var(--radius-xl, 12px);
        font-size: 0.9em;
      }

      .crap-custom-node__label {
        font-weight: 600;
        font-size: 0.75em;
        text-transform: uppercase;
        letter-spacing: 0.05em;
        color: var(--color-primary, #1677ff);
        white-space: nowrap;
      }

      .crap-custom-node__attrs {
        font-size: 0.85em;
        color: var(--text-secondary, rgba(0, 0, 0, 0.65));
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }

      /* -- Node edit modal -- */

      .crap-node-modal {
        position: fixed;
        inset: 0;
        z-index: 10000;
        display: flex;
        align-items: center;
        justify-content: center;
      }

      .crap-node-modal__backdrop {
        position: absolute;
        inset: 0;
        background: rgba(0, 0, 0, 0.3);
      }

      .crap-node-modal__dialog {
        position: relative;
        width: 400px;
        max-width: 90vw;
        max-height: 80vh;
        overflow-y: auto;
        background: var(--surface-primary, #fff);
        border-radius: var(--radius-md, 6px);
        box-shadow: var(--shadow-lg, 0 8px 24px rgba(0,0,0,0.12));
      }

      .crap-node-modal__header {
        padding: var(--space-lg, 1rem) 20px;
        font-weight: 600;
        font-size: 1.05em;
        border-bottom: 1px solid var(--border-color, #e0e0e0);
      }

      .crap-node-modal__body {
        padding: var(--space-lg, 1rem) 20px;
        display: flex;
        flex-direction: column;
        gap: var(--space-md, 0.75rem);
      }

      .crap-node-modal__field {
        display: flex;
        flex-direction: column;
        gap: var(--space-xs, 0.25rem);
      }

      .crap-node-modal__label {
        font-size: 0.85em;
        font-weight: 500;
        color: var(--text-secondary, rgba(0, 0, 0, 0.65));
      }

      .crap-node-modal__input,
      .crap-node-modal__field select,
      .crap-node-modal__field textarea {
        padding: 6px 10px;
        border: 1px solid var(--input-border, #e0e0e0);
        border-radius: var(--radius-sm, 4px);
        font-family: inherit;
        font-size: 0.9em;
        background: var(--input-bg, #fff);
        color: var(--text-primary, rgba(0, 0, 0, 0.88));
      }

      .crap-node-modal__input:focus,
      .crap-node-modal__field select:focus,
      .crap-node-modal__field textarea:focus {
        outline: none;
        border-color: var(--color-primary, #1677ff);
        box-shadow: 0 0 0 2px var(--color-primary-bg, rgba(22, 119, 255, 0.06));
      }

      .crap-node-modal__checkbox {
        display: flex;
        align-items: center;
        gap: var(--space-sm, 0.5rem);
        font-size: 0.9em;
        cursor: pointer;
      }

      .crap-node-modal__footer {
        display: flex;
        justify-content: flex-end;
        gap: var(--space-sm, 0.5rem);
        padding: var(--space-md, 0.75rem) 20px;
        border-top: 1px solid var(--border-color, #e0e0e0);
      }

      .crap-node-modal__btn {
        all: unset;
        padding: 6px 16px;
        border-radius: var(--radius-sm, 4px);
        font-family: inherit;
        font-size: 0.85em;
        font-weight: 500;
        cursor: pointer;
      }

      .crap-node-modal__btn--cancel {
        color: var(--text-secondary, rgba(0, 0, 0, 0.65));
      }

      .crap-node-modal__btn--cancel:hover {
        background: var(--bg-hover, rgba(0, 0, 0, 0.04));
      }

      .crap-node-modal__btn--ok {
        background: var(--color-primary, #1677ff);
        color: #fff;
      }

      .crap-node-modal__btn--ok:hover {
        opacity: 0.9;
      }
    `;
  }
}

/**
 * ProseMirror NodeView for custom nodes. Renders as a styled card (block)
 * or pill (inline) in the editor. Double-click opens edit modal.
 */
class CustomNodeView {
  /**
   * @param {any} node - ProseMirror node
   * @param {any} view - EditorView
   * @param {() => number} getPos - position getter
   * @param {object} nodeDef - custom node definition
   */
  constructor(node, view, getPos, nodeDef) {
    this.node = node;
    this.view = view;
    this.getPos = getPos;
    this.nodeDef = nodeDef;

    this.dom = document.createElement(nodeDef.inline ? 'span' : 'div');
    this.dom.className = `crap-custom-node${nodeDef.inline ? ' crap-custom-node--inline' : ''}`;
    this.dom.contentEditable = 'false';
    this._render();

    this.dom.addEventListener('dblclick', (e) => {
      e.preventDefault();
      e.stopPropagation();
      // Find the CrapRichtext host element
      const host = this._findHost();
      if (host) {
        host._openNodeEditModal(nodeDef, { ...this.node.attrs }, this.getPos());
      }
    });
  }

  /** @returns {CrapRichtext|null} */
  _findHost() {
    let el = this.view.dom;
    while (el) {
      if (el.getRootNode && el.getRootNode().host instanceof CrapRichtext) {
        return el.getRootNode().host;
      }
      el = el.parentElement;
    }
    return null;
  }

  _render() {
    const label = this.nodeDef.label || this.nodeDef.name;
    const attrSummary = (this.nodeDef.attrs || [])
      .slice(0, 3)
      .map(a => this.node.attrs[a.name])
      .filter(v => v != null && v !== '')
      .join(' | ');
    this.dom.innerHTML =
      `<span class="crap-custom-node__label">${label}</span>` +
      (attrSummary ? `<span class="crap-custom-node__attrs">${attrSummary}</span>` : '');
  }

  /**
   * Called by ProseMirror when the node is updated.
   * @param {any} node
   * @returns {boolean}
   */
  update(node) {
    if (node.type.name !== this.nodeDef.name) return false;
    this.node = node;
    this._render();
    return true;
  }

  selectNode() {
    this.dom.classList.add('ProseMirror-selectednode');
  }

  deselectNode() {
    this.dom.classList.remove('ProseMirror-selectednode');
  }

  stopEvent() {
    return true;
  }
}

customElements.define('crap-richtext', CrapRichtext);
