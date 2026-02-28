# Collapsible

Layout-only collapsible container for sub-fields. Like [Row](row.md), sub-fields are promoted as top-level columns with no prefix. Unlike [Group](group.md), which creates prefixed columns (`group__subfield`), Collapsible is purely a UI container.

## Storage

Collapsible fields do **not** create their own column. Each sub-field becomes a top-level column using its plain name — no prefix is added. This is identical to [Row](row.md) storage.

For example, a collapsible with sub-fields `meta_title` and `meta_description` creates columns:
- `meta_title TEXT`
- `meta_description TEXT`

## Definition

```lua
{
    name = "seo_section",
    type = "collapsible",
    admin = {
        label = "SEO Settings",
        collapsed = true,  -- start collapsed in admin UI
    },
    fields = {
        { name = "meta_title", type = "text" },
        { name = "meta_description", type = "textarea" },
    },
}
```

## API Representation

In API responses, collapsible sub-fields appear as flat top-level fields (not nested):

```json
{
  "meta_title": "My Page Title",
  "meta_description": "A description for search engines"
}
```

## Writing Data

Use the plain sub-field names directly — no prefix needed:

```json
{
  "meta_title": "My Page Title",
  "meta_description": "A description for search engines"
}
```

## Admin Rendering

Sub-fields are rendered inside a collapsible section with a toggle header. The section can start collapsed via `admin.collapsed = true`. Clicking the header toggles visibility. This is useful for grouping related fields that don't need to be visible at all times (e.g., SEO settings, advanced options).

## Comparison with Group and Row

| Feature | Group | Row | Collapsible |
|---------|-------|-----|-------------|
| Column prefix | `group__subfield` | none | none |
| API nesting | nested object | flat | flat |
| Admin layout | collapsible fieldset | horizontal row | collapsible section |
| Use case | Namespaced fields | Side-by-side fields | Toggleable sections |
