# Command-Line Reference

```
crap-cms <COMMAND> [OPTIONS]
```

Use `crap-cms --help` to list all commands, or `crap-cms <command> --help` for details on a specific command.

## Global Flags

| Flag | Description |
|------|-------------|
| `-V`, `--version` | Print version and exit |
| `-h`, `--help` | Print help |

## Commands

### `serve` — Start the server

```bash
crap-cms serve <CONFIG> [-d]
```

| Argument / Flag | Description |
|-----------------|-------------|
| `<CONFIG>` | Path to the config directory |
| `-d`, `--detach` | Run in the background (prints PID and exits) |

```bash
crap-cms serve ./my-project
crap-cms serve ./my-project -d
```

### `status` — Show project status

```bash
crap-cms status <CONFIG>
```

Prints collections (with row counts), globals, DB size, and migration status.

```bash
crap-cms status ./my-project
```

### `user` — User management

All user subcommands require a config directory as the first positional argument.

#### `user create`

```bash
crap-cms user create <CONFIG> [-c <COLLECTION>] [-e <EMAIL>] [-p <PASSWORD>] [-f <KEY=VALUE>]...
```

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--collection` | `-c` | `users` | Auth collection slug |
| `--email` | `-e` | — | User email (prompted if omitted) |
| `--password` | `-p` | — | User password (prompted if omitted) |
| `--field` | `-f` | — | Extra fields as key=value (repeatable) |

```bash
# Interactive (prompts for password)
crap-cms user create ./my-project -e admin@example.com

# Non-interactive
crap-cms user create ./my-project \
    -e admin@example.com \
    -p secret123 \
    -f role=admin \
    -f name="Admin User"
```

#### `user list`

```bash
crap-cms user list <CONFIG> [-c <COLLECTION>]
```

Lists all users with ID, email, locked status, and verified status (if email verification is enabled).

```bash
crap-cms user list ./my-project
crap-cms user list ./my-project -c admins
```

#### `user delete`

```bash
crap-cms user delete <CONFIG> [-c <COLLECTION>] [-e <EMAIL>] [--id <ID>] [-y]
```

| Flag | Short | Description |
|------|-------|-------------|
| `--collection` | `-c` | Auth collection slug (default: `users`) |
| `--email` | `-e` | User email |
| `--id` | — | User ID |
| `--confirm` | `-y` | Skip confirmation prompt |

#### `user lock` / `user unlock`

```bash
crap-cms user lock <CONFIG> [-c <COLLECTION>] [-e <EMAIL>] [--id <ID>]
crap-cms user unlock <CONFIG> [-c <COLLECTION>] [-e <EMAIL>] [--id <ID>]
```

#### `user change-password`

```bash
crap-cms user change-password <CONFIG> [-c <COLLECTION>] [-e <EMAIL>] [--id <ID>] [-p <PASSWORD>]
```

### `init` — Scaffold a new config directory

```bash
crap-cms init [DIR]
```

Creates a config directory with `crap.toml`, `init.lua`, `.luarc.json`, `.gitignore`, and empty subdirectories. Defaults to `./crap-cms` if no directory is given.

```bash
crap-cms init ./my-project
```

### `make` — Generate scaffolding files

#### `make collection`

```bash
crap-cms make collection <CONFIG> <SLUG> [-F <FIELDS>] [-T] [-f]
```

| Flag | Short | Description |
|------|-------|-------------|
| `--fields` | `-F` | Inline field shorthand (e.g., `"title:text:required,body:textarea"`) |
| `--no-timestamps` | `-T` | Set `timestamps = false` |
| `--force` | `-f` | Overwrite existing file |

```bash
crap-cms make collection ./my-project posts
crap-cms make collection ./my-project articles -F "title:text:required,body:richtext"
```

#### `make global`

```bash
crap-cms make global <CONFIG> <SLUG> [-f]
```

```bash
crap-cms make global ./my-project site_settings
```

#### `make hook`

```bash
crap-cms make hook <CONFIG> <NAME>
```

Name format: `module.function` (e.g., `posts.auto_slug`).

```bash
crap-cms make hook ./my-project posts.auto_slug
```

#### `make migration`

```bash
crap-cms make migration <CONFIG> <NAME>
```

```bash
crap-cms make migration ./my-project backfill_slugs
```

### `blueprint` — Manage saved blueprints

#### `blueprint save`

```bash
crap-cms blueprint save <CONFIG> <NAME> [-f]
```

Saves a config directory as a reusable blueprint (excluding `data/`, `uploads/`, `types/`).

#### `blueprint use`

```bash
crap-cms blueprint use <NAME> [DIR]
```

Creates a new project from a saved blueprint.

#### `blueprint list`

```bash
crap-cms blueprint list
```

#### `blueprint remove`

```bash
crap-cms blueprint remove <NAME>
```

### `db` — Database tools

#### `db console`

```bash
crap-cms db console <CONFIG>
```

Opens an interactive `sqlite3` session on the project database.

### `export` — Export collection data

```bash
crap-cms export <CONFIG> [-c <COLLECTION>] [-o <FILE>]
```

| Flag | Short | Description |
|------|-------|-------------|
| `--collection` | `-c` | Export only this collection (default: all) |
| `--output` | `-o` | Output file (default: stdout) |

```bash
crap-cms export ./my-project
crap-cms export ./my-project -c posts -o posts.json
```

### `import` — Import collection data

```bash
crap-cms import <CONFIG> <FILE> [-c <COLLECTION>]
```

| Flag | Short | Description |
|------|-------|-------------|
| `--collection` | `-c` | Import only this collection (default: all in file) |

```bash
crap-cms import ./my-project backup.json
crap-cms import ./my-project backup.json -c posts
```

### `typegen` — Generate typed definitions

```bash
crap-cms typegen <CONFIG> [-l <LANG>]
```

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--lang` | `-l` | `lua` | Output language: `lua`, `ts`, `go`, `py`, `rs`, `all` |

```bash
crap-cms typegen ./my-project
crap-cms typegen ./my-project -l all
```

### `proto` — Export proto file

```bash
crap-cms proto [-o <PATH>]
```

Writes `content.proto` to stdout or the given path. No config directory needed.

```bash
crap-cms proto
crap-cms proto -o ./proto/
```

### `migrate` — Run database migrations

```bash
crap-cms migrate <CONFIG> <up|down|list|fresh>
```

| Subcommand | Description |
|------------|-------------|
| `up` | Sync schema + run pending migrations |
| `down [-s N]` | Roll back last N migrations (default: 1) |
| `list` | Show all migration files with status |
| `fresh [-y]` | Drop all tables and recreate (destructive, requires `-y`) |

```bash
crap-cms migrate ./my-project up
crap-cms migrate ./my-project list
crap-cms migrate ./my-project down -s 2
crap-cms migrate ./my-project fresh -y
```

### `backup` — Backup database

```bash
crap-cms backup <CONFIG> [-o <DIR>] [-i]
```

| Flag | Short | Description |
|------|-------|-------------|
| `--output` | `-o` | Output directory (default: `<config>/backups/`) |
| `--include-uploads` | `-i` | Also compress the uploads directory |

```bash
crap-cms backup ./my-project
crap-cms backup ./my-project -o /tmp/backups -i
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Controls log verbosity. Default: `crap_cms=debug,info`. Example: `RUST_LOG=crap_cms=trace` |
