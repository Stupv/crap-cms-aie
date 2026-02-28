# Tabs

Layout-only tabbed container for sub-fields. Like [Row](row.md) and [Collapsible](collapsible.md), sub-fields across all tabs are promoted as top-level columns with no prefix.

## Storage

Tabs fields do **not** create their own column. Each sub-field across all tabs becomes a top-level column using its plain name — no prefix is added. This is identical to [Row](row.md) storage.

For example, tabs with sub-fields `title`, `body`, `meta_title`, and `meta_description` creates columns:
- `title TEXT`
- `body TEXT`
- `meta_title TEXT`
- `meta_description TEXT`

## Definition

```lua
{
    name = "content_tabs",
    type = "tabs",
    tabs = {
        {
            label = "Content",
            fields = {
                { name = "title", type = "text", required = true },
                { name = "body", type = "richtext" },
            },
        },
        {
            label = "SEO",
            description = "Search engine optimization settings",
            fields = {
                { name = "meta_title", type = "text" },
                { name = "meta_description", type = "textarea" },
            },
        },
    },
}
```

Each tab has:
- `label` (required) — the tab button text
- `description` (optional) — help text shown inside the tab panel
- `fields` — array of field definitions (same syntax as any other field list)

## API Representation

In API responses, all tab sub-fields appear as flat top-level fields (not nested by tab):

```json
{
  "title": "My Post",
  "body": "<p>Content here</p>",
  "meta_title": "My Post | Blog",
  "meta_description": "Read about my post"
}
```

## Writing Data

Use the plain sub-field names directly — tabs are invisible at the data layer:

```json
{
  "title": "My Post",
  "body": "<p>Content here</p>",
  "meta_title": "My Post | Blog",
  "meta_description": "Read about my post"
}
```

## Admin Rendering

Sub-fields are organized into tabs with a tab bar at the top. The first tab is active by default. Clicking a tab button switches the visible panel. Each tab can have its own description text.

## Comparison with Other Layout Types

| Feature | Group | Row | Collapsible | Tabs |
|---------|-------|-----|-------------|------|
| Column prefix | `group__subfield` | none | none | none |
| API nesting | nested object | flat | flat | flat |
| Admin layout | collapsible fieldset | horizontal row | collapsible section | tabbed panels |
| Use case | Namespaced fields | Side-by-side fields | Toggleable sections | Organized sections |
