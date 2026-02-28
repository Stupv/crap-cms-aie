# Row

Layout-only grouping of sub-fields. Unlike [Group](group.md), sub-fields are promoted as top-level columns with no prefix.

## Storage

Row fields do **not** create their own column. Each sub-field becomes a top-level column using its plain name — no prefix is added.

For example, a row with sub-fields `firstname` and `lastname` creates columns:
- `firstname TEXT`
- `lastname TEXT`

This is different from [Group](group.md), which prefixes sub-field columns (`seo__title`).

## Definition

```lua
{
    name = "name_row",
    type = "row",
    fields = {
        { name = "firstname", type = "text", required = true },
        { name = "lastname", type = "text", required = true },
    },
}
```

## API Representation

In API responses, row sub-fields appear as flat top-level fields (not nested):

```json
{
  "firstname": "Jane",
  "lastname": "Doe"
}
```

## Writing Row Data

Use the plain sub-field names directly — no prefix needed:

```json
{
  "firstname": "Jane",
  "lastname": "Doe"
}
```

## Admin Rendering

Sub-fields are rendered in a horizontal row layout. The row itself has no fieldset, legend, or collapsible wrapper — it is purely a layout mechanism for placing related fields side by side.
