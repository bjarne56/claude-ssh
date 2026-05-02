# CLAUDE.md - Coding Constraints

## General

- **Naming**: `camelCase` for variables, `PascalCase` for types, descriptive names, no abbreviations except common ones (ctx/err/id/db/tx/req/res)
- **Errors**: Always handle, never ignore; wrap with context; lowercase messages without punctuation
- **Security**: Parameterized queries, no hardcoded secrets, bcrypt for passwords
- **Resources**: Always close (defer/with/try-with-resources/finally)
- **Logging**: Debug/Info/Warn/Error levels; structured fields (trace_id, duration_ms); no sensitive data
- **Tests**: Table-driven, cover edge cases, ≥80% coverage for core logic

---

## Go

```go
// Context as first param, never store in struct
func GetByID(ctx context.Context, id int64) (*Item, error)

// Wrap errors
if err != nil {
    return fmt.Errorf("get item: %w", err)
}

// Defer close, check rows.Err()
defer rows.Close()

// Goroutine must have exit path
go func() {
    select {
    case <-ctx.Done(): return
    case data := <-ch: process(data)
    }
}()

// Preallocate slices, use strings.Builder
buf := make([]byte, 0, size)
```

---

## Rust

```rust
// Result + ? propagation, no .unwrap() in prod
fn get_item(id: i64) -> Result<Item, Error> {
    let item = db.query(id)?;
    Ok(item)
}

// Prefer borrowing over cloning
fn process(data: &Data) -> Result<()>

// Explicit lifetimes only when necessary
fn parse<'a>(input: &'a str) -> &'a str

// unsafe must have safety comment
// SAFETY: pointer is valid and aligned
unsafe { ptr.read() }
```

---

## Python

```python
# Type hints required
def get_item(item_id: int) -> Item | None:

# Specific exceptions, no bare except
try:
    result = fetch()
except ConnectionError as e:
    logger.error("fetch failed", error=e)
    raise

# Context managers for resources
with open(path) as f:
    data = f.read()

# async/await for IO
async def fetch_data() -> Data:
    async with session.get(url) as resp:
        return await resp.json()
```

---

## Java

```java
// try-with-resources
try (var conn = dataSource.getConnection()) {
    // use conn
}

// Optional instead of null
public Optional<Item> findById(long id)

// Specific exceptions
catch (SQLException e) {
    throw new DataAccessException("query failed", e);
}

// Records for DTOs
public record ItemDTO(long id, String name) {}
```

---

## TypeScript

```typescript
// Strict mode, explicit types, no any
function getItem(id: number): Promise<Item>

// async/await
async function fetchData(): Promise<Data> {
  const res = await fetch(url);
  return res.json();
}

// Nullish handling
const name = item?.name ?? "default";

// Type guards
function isError(x: unknown): x is Error {
  return x instanceof Error;
}
```

---

## Role
@AGENTS.md
