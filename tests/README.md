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

## Admin UI Testing

Open http://localhost:3000/admin in a browser. The example config defines
two collections (`posts` and `pages`) with sample field types.

Things to verify:

- Dashboard shows collection cards with item counts
- Collection list shows items in a styled table with pagination
- Create/edit forms render all field types (text, textarea, select, checkbox)
- Delete page shows confirmation dialog
- Empty collections show centered empty state
- 404 page shows styled error with dashboard link
