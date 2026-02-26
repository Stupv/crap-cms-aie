# Array

Repeatable group of sub-fields. Each array item is a row in a join table.

## Storage

Array fields use a dedicated join table: `{collection}_{field}`.

The join table has columns:

| Column | Type | Description |
|--------|------|-------------|
| `id` | TEXT PRIMARY KEY | Nanoid for each row |
| `parent_id` | TEXT NOT NULL | Foreign key to the parent document |
| `_order` | INTEGER NOT NULL | Sort order (0-indexed) |
| *sub-fields* | varies | One column per sub-field |

## Definition

```lua
{
    name = "slides",
    type = "array",
    fields = {
        { name = "title", type = "text", required = true },
        { name = "image_url", type = "text" },
        { name = "caption", type = "textarea" },
    },
    admin = {
        description = "Image slides for the gallery",
    },
}
```

## Sub-Fields

Sub-fields support the same properties as regular fields (name, type, required, default_value, admin, etc.) but do not support nested arrays or relationships.

## API Representation

In API responses, array fields appear as a JSON array of objects:

```json
{
  "slides": [
    { "id": "abc123", "title": "Slide 1", "image_url": "/img/1.jpg", "caption": "First" },
    { "id": "def456", "title": "Slide 2", "image_url": "/img/2.jpg", "caption": "Second" }
  ]
}
```

## Writing Array Data

Via gRPC, pass an array of objects:

```json
{
  "slides": [
    { "title": "Slide 1", "image_url": "/img/1.jpg" },
    { "title": "Slide 2", "image_url": "/img/2.jpg" }
  ]
}
```

On write, all existing rows for the parent are deleted and replaced with the new data. This is a full replacement, not a merge.

## Row Labels

By default, array rows in the admin UI are labeled with the field label and row index (e.g., "Slides 0", "Slides 1"). You can customize this with `label_field` and `row_label`.

### `label_field`

Set `admin.label_field` to the name of a sub-field. Its value is used as the row title, and updates live as you type.

```lua
{
    name = "slides",
    type = "array",
    admin = {
        label_field = "title",
    },
    fields = {
        { name = "title", type = "text", required = true },
        { name = "image_url", type = "text" },
        { name = "caption", type = "textarea" },
    },
}
```

With this configuration, each row shows the `title` value instead of "Slides 0".

### `row_label` (Lua function)

For computed labels, set `admin.row_label` to a Lua function reference. The function receives the row data as a table and returns a display string (or `nil` to fall back to `label_field` or the default).

```lua
-- collections/products.lua
{
    name = "variants",
    type = "array",
    admin = {
        row_label = "labels.variant_row",
        label_field = "name", -- fallback if row_label returns nil
    },
    fields = {
        { name = "name", type = "text", required = true },
        { name = "sku", type = "text" },
        { name = "price", type = "number" },
    },
}
```

```lua
-- hooks/labels.lua
local M = {}

function M.variant_row(row)
    local name = row.name or "Untitled"
    if row.sku and row.sku ~= "" then
        return name .. " (" .. row.sku .. ")"
    end
    return name
end

return M
```

### Priority

1. `row_label` Lua function (if set and returns a non-empty string)
2. `label_field` sub-field value (if set and the field has a value)
3. Default: field label + row index (e.g., "Slides 0")

> **Note:** `row_label` is only evaluated server-side. Rows added via JavaScript in the browser fall back to `label_field` (live-updated) or the default until the form is saved and reloaded.

## Row Limits (`min_rows` / `max_rows`)

Enforce minimum and maximum row counts. These are validation constraints (like `required`), not just UI hints.

```lua
{
    name = "slides",
    type = "array",
    min_rows = 1,
    max_rows = 10,
    fields = { ... },
}
```

- **`min_rows`**: Minimum number of items. Validated on create/update (skipped for draft saves).
- **`max_rows`**: Maximum number of items. Validated on create/update. The admin UI disables the "Add" button when the limit is reached.

Validation runs in `validate_fields()`, shared by admin handlers, gRPC, and Lua `crap.collections.create()`/`update()`.

## Default Collapsed State (`init_collapsed`)

Render existing rows collapsed by default on page load. New rows added via the "Add" button are always expanded.

```lua
{
    name = "slides",
    type = "array",
    admin = {
        init_collapsed = true,
    },
    fields = { ... },
}
```

## Custom Labels (`labels`)

Customize the "Add Row" button text and field header with singular/plural labels.

```lua
{
    name = "slides",
    type = "array",
    admin = {
        labels = { singular = "Slide", plural = "Slides" },
    },
    fields = { ... },
}
```

With this config, the add button reads "Add Slide" instead of "Add Row".

## Admin Rendering

Renders as a repeatable fieldset with:
- Drag handle for drag-and-drop reordering
- Row count badge showing the number of items
- Collapse/expand all buttons
- Each row has expand/collapse toggle, move up/down, duplicate, and remove buttons
- "No items yet" empty state when no rows exist
- "Add Row" button (or custom label) to append new rows
- Add button disabled when `max_rows` is reached
