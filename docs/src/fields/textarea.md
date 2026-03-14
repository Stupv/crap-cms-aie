# Textarea

Multi-line text field for longer content.

## SQLite Storage

`TEXT` column.

## Definition

```lua
crap.fields.textarea({
    name = "description",
    admin = {
        placeholder = "Enter a description...",
        rows = 12,
        resizable = false,
    },
})
```

## Admin Options

| Option | Type | Default | Description |
|---|---|---|---|
| `rows` | integer | `8` | Number of visible rows |
| `resizable` | boolean | `true` | Allow vertical resize via drag handle |

## Admin Rendering

Renders as a `<textarea>` element. By default, the textarea is vertically resizable.
Set `admin.resizable = false` to disable the resize handle.
