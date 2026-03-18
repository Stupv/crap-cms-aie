# Testing

## Prerequisites

- [Rust toolchain](https://rustup.rs/)
- [grpcurl](https://github.com/fullstorydev/grpcurl) — for gRPC API testing

## Running the Server

```bash
cargo build
crap-cms serve ./example
```

This starts two servers:

| Service   | Port    | URL                          |
|-----------|---------|------------------------------|
| Admin UI  | `3000`  | http://localhost:3000/admin   |
| gRPC API  | `50051` | localhost:50051               |

Ports are configurable via `example/crap.toml`.

## gRPC API Testing

`api.sh` contains grpcurl commands wrapped as shell functions for every
ContentAPI endpoint. The server must be running first.

### Setup

Source the file to load the functions into your shell:

```bash
source tests/api.sh
```

### Available Functions

#### Discovery

```bash
list_services                          # list all gRPC services
describe_api                           # describe ContentAPI methods
describe_message crap.FindRequest      # describe a specific message type
```

#### Find

```bash
find_posts                             # list all posts
find_posts_paginated                   # list posts with limit/offset
find_posts_published                   # filter posts by status=published
find_posts_ordered                     # list posts ordered by title
find_pages                             # list all pages
```

#### FindByID

```bash
find_post_by_id <id>                   # get a single post
find_page_by_id <id>                   # get a single page
```

#### Create

```bash
create_post                            # create a draft post
create_post_published                  # create a published post
create_page                            # create a page
```

#### Update

```bash
update_post <id>                       # update a post
update_page <id>                       # update a page
```

#### Delete

```bash
delete_post <id>                       # delete a post
delete_page <id>                       # delete a page
```

### Example Workflow

```bash
source tests/api.sh

# Create a post and grab the ID from the response
create_post

# List all posts to see it
find_posts

# Update it (paste the ID from the create response)
update_post abc123

# Delete it
delete_post abc123
```

### Custom Server Address

The default address is `localhost:50051`. To override:

```bash
source tests/api.sh
ADDR="localhost:9090"
find_posts
```

## E2E Tests

The `tests/e2e/` directory contains end-to-end tests that exercise the full admin UI
stack — routing, template rendering, form submission, database round-trips — without
starting a real HTTP server. Each test builds an in-memory `TestApp` with a temporary
SQLite database, sends requests through the Axum router directly, and asserts on the
HTML response or database state.

### Running

```bash
# Run all e2e tests
cargo test --test e2e

# Run a specific test module
cargo test --test e2e html_nesting

# Run a single test by name
cargo test --test e2e double_nested_group_array_crud_roundtrip
```

Browser-based tests (using headless Chrome via `chromiumoxide`) are behind a feature
flag and excluded by default:

```bash
cargo test --test e2e --features browser-tests
```

### Architecture

```
tests/e2e/
  main.rs          # Test crate root — declares all modules
  helpers.rs       # TestApp, setup_app(), create_test_user(), auth helpers
  html.rs          # HTML assertion helpers (parse, select, assert_exists, count, etc.)
  html_forms.rs    # Form rendering tests (field types, groups, layouts)
  html_nesting.rs  # Nested field tests (arrays, blocks, groups, locale combinations)
  html_crud.rs     # Create/edit/delete round-trips
  html_auth.rs     # Authentication and access control
  html_validation.rs  # Server-side validation error rendering
  html_locale.rs   # Locale switching, locale-locked fields
  html_versions.rs # Draft/publish versioning
  html_globals.rs  # Global definitions
  browser*.rs      # Browser tests (feature-gated, require headless Chrome)
```

### How Tests Work

1. **`setup_app(collections, globals)`** creates a `TestApp` with:
   - A `tempfile::TempDir` containing a fresh SQLite database
   - All collections/globals registered and migrated (tables + join tables)
   - A full Axum router with templates, translations, and hook runner
   - Auth disabled by default (`require_auth = false`)

2. **`setup_app_with_config(..., config)`** is the same but accepts a custom `CrapConfig`
   (used for locale-enabled tests, custom auth settings, etc.).

3. **Requests** are sent directly to the router using `tower::ServiceExt::oneshot()`:

   ```rust
   let resp = app.router.clone().oneshot(
       Request::builder()
           .uri("/admin/collections/posts/create")
           .header("cookie", &cookie)
           .body(Body::empty())
           .unwrap(),
   ).await.unwrap();
   let body = body_string(resp.into_body()).await;
   ```

4. **HTML assertions** use the `scraper` crate via `html.rs` helpers:

   ```rust
   let doc = html::parse(&body);
   html::assert_exists(&doc, "input[name=\"title\"]", "title input should exist");
   html::assert_not_exists(&doc, ".form__error", "should have no errors");
   html::assert_input(&doc, "title", "text", Some("Hello"));
   assert_eq!(html::count(&doc, ".form__array-row"), 2);
   ```

5. **Database verification** for complex scenarios (join tables, locale-scoped data)
   uses the `crap_cms::db::query` API directly:

   ```rust
   let doc = crap_cms::db::query::find_by_id(&conn, "posts", &def, &id, locale_ctx.as_ref())
       .unwrap()
       .expect("document should exist");
   assert_eq!(doc.fields["config"]["items"][0]["name"], "Item1");
   ```

### Writing a New E2E Test

1. **Define the collection** — create a builder function that returns a
   `CollectionDefinition` with the fields you need:

   ```rust
   fn make_my_test_def() -> CollectionDefinition {
       let mut def = CollectionDefinition::new("widgets");
       def.labels = Labels {
           singular: Some(LocalizedString::Plain("Widget".to_string())),
           plural: Some(LocalizedString::Plain("Widgets".to_string())),
       };
       def.timestamps = true;
       def.fields = vec![
           FieldDefinition::builder("title", FieldType::Text)
               .required(true)
               .build(),
           FieldDefinition::builder("items", FieldType::Array)
               .fields(vec![
                   FieldDefinition::builder("name", FieldType::Text).build(),
               ])
               .build(),
       ];
       def
   }
   ```

2. **Set up the app** — always include `make_users_def()` if you need auth:

   ```rust
   #[tokio::test]
   async fn my_test() {
       let app = setup_app(vec![make_my_test_def(), make_users_def()], vec![]);
       let user_id = create_test_user(&app, "test@test.com", "pass123");
       let cookie = make_auth_cookie(&app, &user_id, "test@test.com");
       // ...
   }
   ```

3. **For locale tests**, use `setup_app_with_config` with a locale-enabled config:

   ```rust
   let mut config = CrapConfig::default();
   config.locale = LocaleConfig {
       default_locale: "en".to_string(),
       locales: vec!["en".to_string(), "de".to_string()],
       fallback: false,
   };
   let app = setup_app_with_config(vec![...], vec![], config);
   ```

4. **For join table data** (arrays, blocks, relationships inside groups), save via
   the query API after creating the document:

   ```rust
   let mut conn = app.pool.get().unwrap();
   let tx = conn.transaction().unwrap();
   let doc = crap_cms::db::query::create(&tx, "widgets", &def, &data, None).unwrap();
   crap_cms::db::query::save_join_table_data(
       &tx, "widgets", &def.fields, &doc.id, &join_data, locale_ctx.as_ref(),
   ).unwrap();
   tx.commit().unwrap();
   ```

5. **Add the module** to `main.rs` if you created a new file:

   ```rust
   mod my_new_tests;
   ```

### Conventions

- Test functions are numbered with comments (e.g. `// 42. Group > Group > Array: CRUD roundtrip`)
  within each module to make it easy to reference specific tests.
- Each test module has its own definition builders at the top, followed by shared helpers
  (e.g. `get_create_form`, `post_create`), then the test functions.
- Use `_cookie` (underscore prefix) when the cookie is needed for setup but not used
  directly in assertions.
- Prefer database-level verification (`find_by_id`) over HTML body parsing for complex
  nested data (arrays inside groups, locale-scoped join tables).
- For locale-locked field assertions, check for `.form__locale-badge` (badge shown) and
  absence of `[data-action="add-array-row"]` / `[data-action="add-block-row"]` (controls hidden).

## Admin UI Manual Testing

Open http://localhost:3000/admin in a browser. The example config defines
two collections (`posts` and `pages`) with sample field types.

Things to verify:

- Dashboard shows collection cards with item counts
- Collection list shows items in a styled table with pagination
- Create/edit forms render all field types (text, textarea, select, checkbox)
- Delete page shows confirmation dialog
- Empty collections show centered empty state
- 404 page shows styled error with dashboard link
