# Population Depth

The `depth` parameter controls how deeply relationship fields are populated with full document objects.

## Depth Values

| Depth | Behavior |
|-------|----------|
| `0` | IDs only. Has-one = string ID. Has-many = array of string IDs. |
| `1` | Populate immediate relationships. Replace IDs with full document objects. |
| `2+` | Recursively populate relationships within populated documents. |

## Defaults

| Operation | Default Depth |
|-----------|--------------|
| `Find` (gRPC) | `0` (avoids N+1 on list queries) |
| `FindByID` (gRPC) | `depth.default_depth` from `crap.toml` (default: `1`) |
| `crap.collections.find()` (Lua) | `0` |
| `crap.collections.find_by_id()` (Lua) | `0` |

## Configuration

### Global Config

```toml
[depth]
default_depth = 1   # Default for FindByID (default: 1)
max_depth = 10       # Hard cap for all requests (default: 10)
```

### Per-Field Max Depth

Cap the depth for a specific relationship field, regardless of the request-level depth:

```lua
{
    name = "author",
    type = "relationship",
    relationship = {
        collection = "users",
        max_depth = 1,  -- never populate deeper than 1, even if depth=5
    },
}
```

## Usage

### gRPC

```bash
# Find with depth=1
grpcurl -plaintext -d '{
    "collection": "posts",
    "depth": 1
}' localhost:50051 crap.ContentAPI/Find

# FindByID with depth=2
grpcurl -plaintext -d '{
    "collection": "posts",
    "id": "abc123",
    "depth": 2
}' localhost:50051 crap.ContentAPI/FindByID
```

### Lua API

```lua
-- Find with depth
local result = crap.collections.find("posts", { depth = 1 })

-- FindByID with depth
local post = crap.collections.find_by_id("posts", id, { depth = 2 })
```

## Circular Reference Protection

The population algorithm tracks visited `(collection, id)` pairs. If a document has already been visited in the current recursion path, it's kept as a plain ID string instead of being populated again.

This prevents infinite loops when collections reference each other (e.g., posts → users → posts).

## Performance Considerations

**Use `depth=0` whenever possible.** Population with `depth >= 1` triggers additional queries for every relationship field on every document. This can get very slow, very fast.

- `depth=0` requires no extra queries — always prefer this for list endpoints
- `depth=1` on a `Find` returning 50 documents with 3 relationship fields = up to 150 extra queries
- `depth=2+` compounds this — each populated document's relationships are also populated, leading to exponential query growth
- Has-many relationships use batch `IN` queries (one query per field regardless of ID count)
- Has-one relationships use one query per field per document

**Recommendations:**

- Use `select` to limit which fields are returned — non-selected relationship fields are skipped during population
- Set `max_depth` on relationship fields that don't need deep population
- For list views, use `depth=0` and fetch related data only when displaying a single document
- If you need related data in a list, consider using `depth=1` with `select` to populate only the specific relationship fields you need
- `Find` defaults to `depth=0` for this reason — don't override it without understanding the query cost
