# ADR-006: Cursor-based pagination over OFFSET

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

## Decision

All list endpoints use keyset cursor pagination. The cursor is the `public_id` of the last item returned. Clients pass it as a `cursor` query parameter on the next request. The API returns a `next_cursor` field in the response body (`null` when the last page is reached).

```json
{
  "items": [...],
  "next_cursor": "pwh_aB3kL9mXz",
  "limit": 20
}
```

The cursor is opaque to the client (it happens to be a public ID today, but clients must not depend on this). Future implementations could Base64-encode a composite sort key without a breaking API change.

Sorting is always ascending by `public_id`. Because public IDs are generated with NanoId (URL-safe random characters), they sort consistently as strings.

The `limit` defaults to 20 and is capped at 100.

## Consequences

**Positive:**
- Consistent query performance at any page depth
- No double-rendering or skipped rows under concurrent writes
- Opaque cursor gives flexibility to change the sort key in future

**Negative:**
- Cannot jump to an arbitrary page number — cursors must be followed sequentially
- Cannot sort by arbitrary columns without changing the cursor format (currently locked to public_id order)
- Clients must handle `next_cursor: null` rather than comparing page counts — slightly more complex client code than OFFSET
