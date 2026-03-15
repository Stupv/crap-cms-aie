# Crap CMS

Headless CMS in Rust. Lua config (neovim-style) + gRPC API + HTMX admin UI.

For usage documentation, see the [user manual](https://crapcms.com/docs) (source in `docs/`).

## Tech Stack

| Component    | Technology                            |
|--------------|---------------------------------------|
| Language     | Rust (edition 2021)                   |
| Web / Admin  | Axum + Handlebars + HTMX             |
| API          | gRPC via Tonic + Prost               |
| Database     | SQLite via rusqlite (WAL mode)        |
| Hooks        | Lua 5.4 via mlua                      |
| IDs          | nanoid                                |

## Project Structure

```
src/
├── main.rs           # binary entry point, subcommand dispatch
├── lib.rs            # crate exports
├── config/           # crap.toml loading + defaults
├── core/             # collection, field, document types
├── db/               # SQLite pool, migrations, query builder
├── hooks/            # Lua VM, crap.* API, hook lifecycle
├── admin/            # Axum admin UI (handlers, templates)
├── api/              # Tonic gRPC service
├── scheduler/        # background job scheduler
├── mcp/              # Model Context Protocol server
├── commands/         # CLI subcommands
└── scaffold/         # init/make scaffolding
```

## Development

```bash
git config core.hooksPath .githooks  # enable shared git hooks (fmt + clippy pre-commit)
cargo build                          # compile
cargo test                           # run tests (3600+)
cargo tarpaulin --out html           # coverage report
crap-cms serve ./example             # run with example config
```

Static files and templates are compiled into the binary via `include_dir!`. Rebuild after changing files in `static/` or `templates/`.

Dev mode (`admin.dev_mode = true` in `crap.toml`) reloads templates from disk per-request — but static files still require a rebuild.

### API Testing

Requires [grpcurl](https://github.com/fullstorydev/grpcurl) and a running server:

```bash
source tests/api.sh
find_posts
create_post
```

### Load Testing

#### gRPC benchmarks (recommended)

Requires [ghz](https://github.com/bojand/ghz), grpcurl, protoc, jq, and a running server:

```bash
./tests/grpc_loadtest.sh                              # all scenarios, default settings
./tests/grpc_loadtest.sh --duration 5                 # shorter runs
./tests/grpc_loadtest.sh --concurrency 1,10           # custom concurrency levels
./tests/grpc_loadtest.sh --scenarios find,count        # specific scenarios only
```

Scenarios: `describe`, `count`, `find`, `find_where`, `find_by_id`, `find_deep`, `create`, `update`.

#### HTTP + gRPC mixed

Requires [oha](https://github.com/hatoo/oha), grpcurl, jq, and a running server:

```bash
./tests/loadtest.sh                                    # all scenarios
./tests/loadtest.sh --scenarios read_list,grpc_find    # specific scenarios
```

### Documentation Book

```bash
cd docs && mdbook build            # build the user manual
cd docs && mdbook serve            # local preview at localhost:3000
```

## License

TBD
