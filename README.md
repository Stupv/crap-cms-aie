# Crap CMS

A headless CMS in Rust. Compiled core + Lua hooks (neovim-style) + overridable HTMX admin UI.

Define collections in Lua, get a gRPC API and admin interface automatically.
Customize behavior with hooks, override the admin UI with templates.
Everything lives in a single config directory — like `~/.config/nvim/` for your CMS.

## Quick Start

```bash
# Build
cargo build

# Run with the example config
crap-cms serve ./example
```

Admin UI: http://localhost:3000/admin
gRPC API: localhost:50051

## How It Works

You point Crap CMS at a config directory. It reads your Lua collection
definitions, creates the database tables, and serves both an admin UI and
a gRPC API.

```
my-site/
├── crap.toml              # server/db settings
├── init.lua               # entry point (like nvim's init.lua)
├── collections/           # collection definitions (auto-loaded)
│   ├── posts.lua
│   └── pages.lua
├── globals/               # singleton documents
├── hooks/                 # hook functions
├── templates/             # admin UI template overrides
├── static/                # additional static assets
├── uploads/               # uploaded files (runtime)
└── data/                  # database (runtime)
    └── crap.db
```

### Defining a Collection

```lua
-- collections/posts.lua
crap.collections.define("posts", {
    labels = { singular = "Post", plural = "Posts" },
    timestamps = true,
    admin = {
        use_as_title = "title",
        default_sort = "-created_at",
    },
    fields = {
        { name = "title",   type = "text",     required = true },
        { name = "slug",    type = "text",     required = true, unique = true },
        { name = "status",  type = "select",   options = {
            { label = "Draft", value = "draft" },
            { label = "Published", value = "published" },
        }},
        { name = "content", type = "textarea" },
    },
    hooks = {
        before_change = { "hooks.posts.auto_slug" },
    },
})
```

### Writing a Hook

```lua
-- hooks/posts.lua
local M = {}

function M.auto_slug(context)
    local data = context.data
    if data.title and (not data.slug or data.slug == "") then
        data.slug = crap.util.slugify(data.title)
    end
    return context
end

return M
```

### Configuration

```toml
# crap.toml
[server]
admin_port = 3000
grpc_port = 50051
host = "0.0.0.0"

[database]
path = "data/crap.db"

[admin]
dev_mode = true    # reload templates on every request
```

## Tech Stack

| Component    | Technology                            |
|--------------|---------------------------------------|
| Language     | Rust (edition 2021)                   |
| Web / Admin  | Axum + Handlebars + HTMX             |
| API          | gRPC via Tonic + Prost               |
| Database     | SQLite via rusqlite (WAL mode)        |
| Hooks        | Lua 5.4 via mlua                      |
| IDs          | nanoid                                |

## gRPC API

The content API uses gRPC with server reflection. All collections share
the same generic endpoints:

| Method       | Description               |
|--------------|---------------------------|
| `Find`       | Query documents (filter, sort, paginate) |
| `FindByID`   | Get a single document     |
| `Create`     | Create a document         |
| `Update`     | Update a document         |
| `Delete`     | Delete a document         |

Test with [grpcurl](https://github.com/fullstorydev/grpcurl):

```bash
# List all posts
grpcurl -plaintext -d '{"collection": "posts"}' localhost:50051 crap.ContentAPI/Find

# Create a post
grpcurl -plaintext -d '{
  "collection": "posts",
  "data": {"title": "Hello", "slug": "hello", "status": "draft"}
}' localhost:50051 crap.ContentAPI/Create
```

See `tests/api.sh` for the full set of test commands.

## Admin UI

Server-rendered with Handlebars + HTMX. No JavaScript build step.

Templates and static assets use an overlay system — drop a file in your
config directory's `templates/` or `static/` folder to override the
compiled defaults. In dev mode, template changes take effect on the next
request without restarting.

## Project Structure

```
src/
├── main.rs           # binary entry point, subcommand dispatch
├── lib.rs            # crate exports
├── config.rs         # crap.toml loading + defaults
├── core/             # collection, field, document types
├── db/               # SQLite pool, migrations, query builder
├── hooks/            # Lua VM, crap.* API, hook lifecycle
├── admin/            # Axum admin UI (handlers, templates)
└── api/              # Tonic gRPC service
```

## Development

```bash
cargo build                        # compile
crap-cms serve ./example           # run with example site
cargo test                         # run tests
```

API testing (requires grpcurl and a running server):

```bash
source tests/api.sh
find_posts
create_post
```

See [tests/README.md](tests/README.md) for the full testing guide.

## Status

Working towards PayloadCMS feature parity. See [CONCEPT.md](CONCEPT.md) for
the full architecture document and [CLAUDE.md](CLAUDE.md) for contributor
conventions and design decisions.

## License

TBD
