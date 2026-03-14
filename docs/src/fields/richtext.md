# Rich Text

Rich text field with a ProseMirror-based WYSIWYG editor. Stored as HTML (default) or
ProseMirror JSON.

## SQLite Storage

`TEXT` column containing HTML content (default) or ProseMirror JSON document.

## Definition

```lua
crap.fields.richtext({
    name = "content",
    admin = {
        placeholder = "Write your content...",
    },
})
```

## Storage Format

By default, richtext fields store raw HTML. Set `admin.format = "json"` to store the
ProseMirror document structure as JSON instead:

```lua
crap.fields.richtext({
    name = "content",
    admin = {
        format = "json",
    },
})
```

### HTML vs JSON

| | HTML (default) | JSON |
|---|---|---|
| Storage | Raw HTML string | ProseMirror `doc.toJSON()` |
| Round-trip fidelity | Loses some structural info | Lossless |
| Programmatic manipulation | Parse HTML | Walk JSON tree |
| FTS search | Indexed as-is | Plain text extracted automatically |
| API response | HTML string | JSON string |

### Important notes

- **Changing format does NOT migrate existing data.** If you switch from `"html"` to
  `"json"` (or vice versa), existing documents retain their original format. The editor
  will attempt to parse the stored content according to the current format setting.
- The API returns the stored format as-is (HTML string or JSON string).
- Full-text search automatically extracts plain text from JSON-format richtext fields.

## Toolbar Configuration

By default, all toolbar features are enabled. Use `admin.features` to limit which
features are available:

```lua
crap.fields.richtext({
    name = "content",
    admin = {
        features = { "bold", "italic", "heading", "link", "bulletList" },
    },
})
```

### Available Features

| Feature | Description |
|---|---|
| `bold` | Bold text (Ctrl+B) |
| `italic` | Italic text (Ctrl+I) |
| `code` | Inline code (Ctrl+\`) |
| `link` | Hyperlinks |
| `heading` | H1, H2, H3 headings |
| `blockquote` | Block quotes |
| `orderedList` | Numbered lists |
| `bulletList` | Bullet lists |
| `codeBlock` | Code blocks (```) |
| `horizontalRule` | Horizontal rule |

When `features` is omitted or empty, all features are enabled (backward compatible).
Undo/redo buttons are always available regardless of feature configuration.

## Custom Nodes

Custom ProseMirror nodes let you embed structured components (CTAs, embeds, alerts,
mentions, etc.) inside richtext content. Register nodes in `init.lua`, then enable
them on specific fields via `admin.nodes`.

### Registration

```lua
-- init.lua
crap.richtext.register_node("cta", {
    label = "Call to Action",
    inline = false, -- block-level node
    attrs = {
        { name = "text", type = "text", label = "Button Text", required = true },
        { name = "url", type = "text", label = "URL", required = true },
        { name = "style", type = "select", label = "Style", options = {
            { label = "Primary", value = "primary" },
            { label = "Secondary", value = "secondary" },
        }},
    },
    searchable_attrs = { "text" },
    render = function(attrs)
        return string.format(
            '<a href="%s" class="btn btn--%s">%s</a>',
            attrs.url, attrs.style or "primary", attrs.text
        )
    end,
})
```

### Field configuration

```lua
crap.fields.richtext({
    name = "content",
    admin = {
        format = "json",
        nodes = { "cta" },
        features = { "bold", "italic", "heading", "link", "bulletList" },
    },
})
```

### Node spec options

| Option | Type | Description |
|---|---|---|
| `label` | string | Display label (defaults to node name) |
| `inline` | boolean | Inline vs block-level (default: false) |
| `attrs` | table[] | Attribute definitions (see below) |
| `searchable_attrs` | string[] | Attr names included in FTS search index |
| `render` | function | Server-side render function: `(attrs) -> html` |

### Attribute types

| Type | Admin Input |
|---|---|
| `text` | Text input |
| `number` | Number input |
| `select` | Dropdown with options |
| `checkbox` | Checkbox |
| `textarea` | Multi-line textarea |

### Server-side rendering

Use `crap.richtext.render(content)` in hooks to replace custom nodes with rendered
HTML. The function auto-detects format (JSON or HTML). Custom nodes with a `render`
function produce the function's output; nodes without one pass through as
`<crap-node>` custom elements.

```lua
-- In an after_read hook
function hooks.render_content(context)
    local doc = context.doc
    if doc.content then
        doc.content = crap.richtext.render(doc.content)
    end
    return context
end
```

### FTS search

Custom node attributes listed in `searchable_attrs` are automatically extracted
for full-text search when the field uses JSON format.

## Resize Behavior

By default, the richtext editor is vertically resizable (no max-height constraint). Set
`admin.resizable = false` to lock it to a fixed height range (200–600px):

```lua
crap.fields.richtext({
    name = "content",
    admin = {
        resizable = false,
    },
})
```

## Admin Rendering

Renders as a ProseMirror-based rich text editor with a configurable toolbar. When
custom nodes are configured, an insert button group appears in the toolbar for each
node type. Nodes display as styled cards (block) or pills (inline) in the editor;
double-click to edit attributes.

## Notes

- No server-side sanitization is applied — sanitize in hooks if needed
- The toolbar configuration only affects the admin UI; it does not validate or strip content server-side
- Custom node names must be alphanumeric with underscores only
