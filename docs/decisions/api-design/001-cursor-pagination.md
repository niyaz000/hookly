# ADR api-design/001: Cursor-based pagination over OFFSET

## Status
Accepted

## Context

All list endpoints need pagination. The two common approaches:

**OFFSET/LIMIT:**
```sql
SELECT ... FROM table ORDER BY id LIMIT 20 OFFSET 40
```
Simple to implement; cursor is just a page number. But:
- `OFFSET 40` forces the database to scan and discard 40 rows — gets slower as pages increase
- Concurrent inserts can cause rows to appear on multiple pages or be skipped entirely (the "missing row" problem)
- Not safe for high-cardinality tables with frequent writes

**Cursor (keyset) pagination:**
```sql
SELECT ... FROM table WHERE id > $cursor ORDER BY id LIMIT 20
```
- O(1) seek via index — performance is constant regardless of page depth
- Stable under concurrent writes — the cursor anchors the position in the sort order
- Opaque cursor token hides implementation details from the client

Hookly's tables (events, delivery attempts, endpoints) are append-heavy and expected to grow large. High-page-number `OFFSET` queries on the events table would degrade noticeably.

### Why not sort by `public_id`?

Public IDs are prefixed NanoIds — 16 random alphanumeric characters. Because they carry no ordering information, a newly created record can receive a `public_id` that sorts lexicographically before thousands of existing records. Sorting by `public_id` produces an undefined, non-deterministic page order.

The correct sort key is the internal `id` column (UUIDv7). UUIDv7 embeds a 48-bit millisecond timestamp as its most-significant bits, making it monotonically increasing over time and safe as a keyset anchor. Sorting by `id` gives stable chronological ordering. The cursor encodes this internal UUID, which keeps the sort key opaque to clients.

## Decision

All list endpoints use keyset cursor pagination anchored on the internal `id` (UUIDv7) column. The cursor is the base64-encoded UUIDv7 of the anchor record. Clients pass it as a `cursor` query parameter. The API returns both `previous_cursor` and `next_cursor` to support bidirectional navigation.

**Forward page (default):**
```sql
SELECT ... FROM table WHERE id > $decoded_cursor ORDER BY id ASC LIMIT $limit
```

**Backward page:**
```sql
SELECT ... FROM table WHERE id < $decoded_cursor ORDER BY id DESC LIMIT $limit
-- results are reversed before returning
```

**Response envelope:**
```json
{
  "data": [...],
  "metadata": {
    "limit": 20,
    "previous_cursor": "MDFKWVhHNUZWUlE4WFhBQ1Q4WFZGNTQ5NA==",
    "next_cursor": "MDFKWVhHNUZWUlE4WFhBQ1Q4WFZGNTQ5NA==",
    "sort": {
      "field": "created_at",
      "direction": "asc"
    }
  }
}
```

- `previous_cursor` is `null` when the client is on the first page (no records precede the current window)
- `next_cursor` is `null` when the last page is reached (fewer than `limit` records were returned)
- The cursor payload is the raw UUIDv7 bytes, base64-encoded — clients must treat it as opaque
- `limit` defaults to 20, capped at 100
- `data` is always an array; an empty last page returns `[]` with `next_cursor: null`

## Principles upheld

- **Frugality** — O(1) index seek at any page depth; `OFFSET` scans compound cost with table size
- **Performance as first-class** — stable performance for large tables like `events` and `delivery_jobs` regardless of how deep the client pages
- **Developer experience** — bidirectional cursors, `null` sentinel for last page, and opaque tokens give clients a consistent contract they can rely on

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| OFFSET/LIMIT | Degrades with table size; skips/duplicates rows under concurrent writes |
| Sort by `public_id` (NanoId) | NanoId is random; new records can sort before old ones — page order undefined |
| Sort by `created_at` alone | Timestamp collisions under concurrent inserts; not a unique sort key without a tiebreaker |
| Expose raw UUIDv7 as cursor | Leaks internal ID format; base64 wrapping allows format to change without a breaking API change |

## Consequences

**Positive:**
- Consistent query performance at any page depth
- Bidirectional navigation without extra round-trips
- Cursor format change (e.g. composite key) is non-breaking behind base64 encoding
- `data` / `metadata` split gives clients a clean contract; metadata is extensible without touching the payload

**Negative:**
- Cannot jump to an arbitrary page number — cursors must be followed sequentially
- Cannot sort by arbitrary columns without changing the cursor format (sort key is currently fixed to `id`)
- Clients must handle `next_cursor: null` rather than comparing page counts — slightly more complex than OFFSET
- **Rows from concurrent transactions may be skipped.** If transaction T1 generates `UUID_c` but commits after transaction T2 (which generated `UUID_d > UUID_c`) and after the client has already fetched the page containing `UUID_d`, then `UUID_c` will never appear: the next page anchors at `id > UUID_d`, and `UUID_c < UUID_d`. The skip window is bounded by the overlap duration of concurrent write transactions — typically milliseconds for simple inserts. This is the accepted price for O(1) page performance. Clients that require guaranteed completeness over a time window should use event streams or polling rather than cursor pagination.
