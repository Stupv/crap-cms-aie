# crap.locale

Read-only access to the locale configuration. Available in `init.lua` and all hook functions.

## Functions

### `crap.locale.get_default()`

Returns the default locale code from `crap.toml`.

```lua
local default = crap.locale.get_default()  -- "en"
```

### `crap.locale.get_all()`

Returns an array of all configured locale codes. Returns an empty table if localization is disabled.

```lua
local locales = crap.locale.get_all()  -- {"en", "de", "fr"}
```

### `crap.locale.is_enabled()`

Returns `true` if localization is enabled (at least one locale configured in `crap.toml`).

```lua
if crap.locale.is_enabled() then
    -- localization is active
end
```

## Example

```lua
-- In a hook: generate localized slugs
function M.before_change(ctx)
    if crap.locale.is_enabled() and ctx.locale then
        ctx.data.slug = crap.util.slugify(ctx.data.title) .. "-" .. ctx.locale
    else
        ctx.data.slug = crap.util.slugify(ctx.data.title)
    end
    return ctx
end
```
