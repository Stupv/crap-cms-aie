# crap.toml

The `crap.toml` file configures the server, database, authentication, and other global settings. All sections and fields are optional — sensible defaults are used when omitted.

If `crap.toml` does not exist in the config directory, all defaults apply.

## Full Reference

```toml
[server]
admin_port = 3000       # Admin UI port
grpc_port = 50051       # gRPC API port
host = "0.0.0.0"        # Bind address

[database]
path = "data/crap.db"   # Relative to config dir, or absolute

[admin]
dev_mode = true          # Reload templates per-request (disable in production)

[auth]
secret = ""              # JWT signing key. Empty = auto-generated (tokens won't survive restarts)
token_expiry = 7200      # Default token expiry in seconds (2 hours)

[depth]
default_depth = 1        # Default population depth for FindByID (Find always defaults to 0)
max_depth = 10           # Hard cap on population depth (prevents abuse)

[upload]
max_file_size = 52428800 # Global max file size in bytes (50 MB)

[email]
smtp_host = ""           # SMTP server hostname. Empty = email disabled (no-op)
smtp_port = 587          # SMTP port (587 for STARTTLS)
smtp_user = ""           # SMTP username
smtp_pass = ""           # SMTP password
from_address = "noreply@example.com"  # Sender email address
from_name = "Crap CMS"  # Sender display name

[hooks]
on_init = []             # Lua function refs to run at startup (with CRUD access)

[live]
enabled = true           # Enable SSE + gRPC Subscribe for live mutation events
channel_capacity = 1024  # Broadcast channel buffer size

[locale]
default_locale = "en"    # Default locale code
locales = ["en", "de"]   # Supported locales (empty = disabled)
fallback = true          # Fall back to default locale if field is NULL
```

## Section Details

### `[server]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `admin_port` | integer | `3000` | Port for the Axum admin UI |
| `grpc_port` | integer | `50051` | Port for the Tonic gRPC API |
| `host` | string | `"0.0.0.0"` | Bind address for both servers |

### `[database]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `path` | string | `"data/crap.db"` | SQLite database path. Relative paths are resolved from the config directory. Absolute paths are used as-is. |

### `[admin]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `dev_mode` | boolean | `true` | When true, templates are reloaded from disk on every request. Set to `false` in production for cached templates. |

### `[auth]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `secret` | string | `""` (empty) | JWT signing secret. If empty, a random 64-character secret is generated at startup. **Set this in production** so tokens survive restarts. |
| `token_expiry` | integer | `7200` | Default JWT token lifetime in seconds. Can be overridden per auth collection. |

### `[depth]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `default_depth` | integer | `1` | Default population depth for `FindByID`. `Find` always defaults to `0`. |
| `max_depth` | integer | `10` | Maximum allowed depth for any request. Hard cap to prevent excessive queries. |

### `[upload]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_file_size` | integer | `52428800` | Global maximum file size in bytes (50 MB). Per-collection `max_file_size` overrides this. |

### `[email]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `smtp_host` | string | `""` (empty) | SMTP server hostname. **Empty = email disabled** — all send attempts log a warning and return Ok. |
| `smtp_port` | integer | `587` | SMTP port. 587 is the standard STARTTLS port. |
| `smtp_user` | string | `""` | SMTP authentication username. |
| `smtp_pass` | string | `""` | SMTP authentication password. |
| `from_address` | string | `"noreply@example.com"` | Sender email address for outgoing mail. |
| `from_name` | string | `"Crap CMS"` | Sender display name. |

When configured, email enables password reset ("Forgot password?" link on login), email verification (optional per-collection), and the `crap.email.send()` Lua API.

### `[hooks]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `on_init` | string[] | `[]` | Lua function refs to execute at startup. These run synchronously with CRUD access — failure aborts startup. |

### `[live]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable live event streaming (SSE + gRPC Subscribe). |
| `channel_capacity` | integer | `1024` | Internal broadcast channel buffer size. Increase if subscribers lag. |

See [Live Updates](../live-updates/overview.md) for full documentation.

### `[locale]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `default_locale` | string | `"en"` | Default locale code. Content without an explicit locale uses this. |
| `locales` | string[] | `[]` (empty) | Supported locale codes. **Empty = localization disabled.** When empty, all fields behave as before (single value, no locale columns). |
| `fallback` | boolean | `true` | When reading a non-default locale, fall back to the default locale value if the requested locale field is NULL. Uses `COALESCE` in SQL. |

When locales are configured, any field with `localized = true` in its Lua definition gets one column per locale (`title__en`, `title__de`) instead of a single `title` column. The API accepts a `locale` parameter on Find, FindByID, Create, Update, GetGlobal, and UpdateGlobal to control which locale to read/write. The admin UI shows a locale selector in the edit sidebar.

**Special locale values:**
- `"all"` — returns all locales as nested objects: `{ title: { en: "Hello", de: "Hallo" } }`
- Any locale code (e.g., `"en"`, `"de"`) — returns flat field names with that locale's values
- Omitted — uses the default locale

## Example

```toml
[server]
admin_port = 8080
grpc_port = 9090
host = "127.0.0.1"

[database]
path = "/var/lib/crap/production.db"

[admin]
dev_mode = false

[auth]
secret = "a-very-long-random-string-for-jwt-signing"
token_expiry = 86400  # 24 hours

[depth]
default_depth = 1
max_depth = 5

[upload]
max_file_size = 104857600  # 100 MB

[email]
smtp_host = "smtp.example.com"
smtp_port = 587
smtp_user = "noreply@example.com"
smtp_pass = "your-smtp-password"
from_address = "noreply@example.com"
from_name = "My App"

[hooks]
on_init = ["hooks.seed.run"]
```
