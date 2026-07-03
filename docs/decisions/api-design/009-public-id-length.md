# ADR api-design/009: Public ID structure — 16-character time-ordered NanoId on a 62-symbol alphabet

## Status
Accepted (revised — see **Revision** section)

## Context

[ADR 002](002-dual-id-strategy.md) establishes that every resource carries a public identifier in the form `<prefix>_<NanoId>`. This ADR fixes the concrete parameters of the NanoId component: which alphabet, how many characters, and the probability argument that justifies the choice.

### Alphabet

The alphabet is the 62 alphanumeric symbols: `0–9`, `A–Z`, `a–z`.

| Property | Value |
|---|---|
| Symbols | 62 |
| URL-safe | Yes — all 62 are RFC 3986 unreserved characters; no percent-encoding required in path segments or query strings |
| Encoding | ASCII-only — safe in HTTP headers, JSON strings, and log pipelines |
| Charset | Digits + uppercase + lowercase — typeable, no special-character quoting issues in shells or SQL |

**Why not base64url (64 symbols)?** Adding `-` and `_` saves 1 character per 60 chars of entropy but introduces symbols that are visually indistinguishable from separators and path delimiters in certain rendering contexts.

**Why not hex (16 symbols)?** 16 symbols would require 40 characters to carry the same entropy as a 16-char base62 string — too long.

**Why not extended ASCII or full printable (94 symbols)?** Symbols like `"`, `<`, `>`, `&` break JSON, HTML, and shell quoting. The marginal entropy gain (≈0.3 bits/char over base62) does not justify the quoting complexity.

### Collision model

NanoId characters are drawn independently and uniformly from the alphabet by a CSPRNG (the `nanoid` crate on Rust delegates to `getrandom`, which reads from the OS entropy pool). The collision risk is the birthday problem:

For a space of **N = 62^L** distinct IDs (where L is the length), the expected number of IDs that must be generated before the probability of at least one collision reaches 50% is:

```
n₅₀ ≈ √(2 × N × ln 2)
```

| Length (L) | Space (62^L) | n₅₀ (IDs before 50% collision) |
|---|---|---|
| 8 | 2.18 × 10¹⁴ | ~17 million |
| 12 | 3.22 × 10²¹ | ~67 billion |
| **16** | **4.74 × 10²⁸** | **~810 trillion** |
| 20 | 7.04 × 10³⁵ | ~1.2 × 10¹⁸ |

**Length 8 is insufficient.** 17 million is reachable on the `events` table of an active platform — a tenant emitting 100 events per second would expect a collision within days. Ruled out.

**Length 12 is borderline.** 67 billion is safe for most tables today. But `events` and `delivery_jobs` are the highest-volume tables in the system, and at multi-year timescales for a large platform the margin is uncomfortably thin. The cost of upgrading from 12 to 16 is four characters per ID; the benefit is four additional orders of magnitude of headroom.

**Length 16 is the chosen value.** 810 trillion IDs before an expected collision. At one million events per second — far beyond any foreseeable load — the expected first collision on the `events` table occurs after more than 25 years of continuous operation. The margin is effectively unconditional.

**Length 20 is unnecessarily long.** The extra four characters over 16 add storage and wire bytes with no corresponding benefit at any realistic scale. The `n₅₀` at length 20 is ~1.2 × 10¹⁸ — roughly 1 500× more than length 16, which itself already has an effectively infinite safety margin.

## Decision

The NanoId component of every `public_id` is **16 characters** on the **62-symbol alphanumeric alphabet** (`0–9A–Za–z`), structured as:

```
[ 8 chars: ms timestamp in base-62 ] [ 8 chars: CSPRNG randomness ]
```

The timestamp prefix is the number of milliseconds since the Unix epoch encoded in base-62 (big-endian, most-significant digit first). The random suffix is 8 characters drawn from the OS entropy pool via the `nanoid` crate.

The full `public_id` is stored in a `VARCHAR` column sized to `len(prefix) + 1 + 16`:
- 3-char prefix (e.g., `ep_`, `dj_`): `VARCHAR(20)`
- 4-char prefix (e.g., `app_`, `evt_`): `VARCHAR(21)`

### Why time-ordered?

The `public_id` column carries a `UNIQUE` index on every resource table. With purely random 16-char NanoIds, every insert scatters to a random position in that B-tree, causing page splits and index bloat at scale — the same problem that motivated UUIDv7 for the internal `id` PK (see [ADR 002](002-dual-id-strategy.md)).

A timestamp-prefix means new IDs are always lexicographically greater than any existing ID, so inserts always append to the rightmost leaf of the `public_id` index — sequential writes, no page splits.

### Collision model (revised)

The random suffix is 8 base-62 characters, giving **62^8 ≈ 218 trillion** combinations within each millisecond bucket. The birthday-paradox threshold within a single millisecond:

```
n₅₀ ≈ √(2 × 62^8 × ln 2) ≈ 17 million inserts/ms
```

The system is designed for a peak throughput of **10 000 inserts/second**, which is at most **10 inserts per millisecond**. The collision probability per millisecond at peak load:

```
P ≈ 10² / (2 × 218 × 10¹²) ≈ 2.3 × 10⁻¹³
```

This is effectively zero. The `UNIQUE` constraint on `public_id` is the definitive safety net — a collision produces a constraint violation, not silent corruption.

| Inserts/ms at peak | Collision probability per ms |
|---|---|
| 10 (10 K/s peak) | ~2.3 × 10⁻¹³ |
| 1 000 (1 M/s) | ~2.3 × 10⁻⁹ |
| 17 000 000 (n₅₀) | ~50% |

### Timestamp headroom

`62^8 ≈ 218 trillion` milliseconds ≈ **6 926 years** from the Unix epoch. The timestamp prefix will not overflow within any foreseeable lifetime of the platform.

## Principles upheld

- **Performance as first-class** — timestamp prefix keeps `public_id` UNIQUE index inserts sequential, eliminating B-tree page splits (consistent with the UUIDv7 rationale for the internal PK)
- **Frugality** — 16 characters total; no increase in storage or wire overhead vs. the original random-only design
- **Security by design** — 8-char random suffix (62^8 ≈ 2^47 bits of entropy) combined with a UNIQUE constraint makes brute-force enumeration computationally infeasible at any realistic scale

## Revision

The original decision (2024) used 16 fully-random characters drawn from the 62-symbol alphabet. This revision replaces the first 8 characters with a millisecond timestamp in base-62, keeping the total length at 16 and the alphabet unchanged.

**What changed:** IDs are now lexicographically monotonic within the same entity type, which keeps the `public_id` UNIQUE index append-only. The random suffix shrinks from 62^16 to 62^8 per ms bucket, which remains safe at the system's target throughput of ≤10 K inserts/sec.

**What did not change:** Total ID length (16 chars), alphabet (62 symbols), storage column widths, API contract (clients must treat `public_id` as an opaque string regardless of internal structure).

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Purely random 16 chars (original) | Random inserts into the `public_id` UNIQUE index fragment the B-tree at scale |
| ULID (base-32, 26 chars) | Longer IDs for the same entropy; base-32 uses a non-alphanumeric alphabet; adds an external dependency |
| UUID v7 as public_id | Exposes a 48-bit timestamp that clients can decode, leaking creation time; 36-char hyphenated format is verbose in URLs |
| 6ts + 10random split | Provides 62^10 ≈ 839 T random combinations/ms; not necessary given ≤10 inserts/ms at peak — 62^8 ≈ 218 T is already orders of magnitude above any realistic load |
| Variable length per resource type | Different validation rules per type add operational complexity; a uniform 16-char rule is simpler |

## Consequences

**Positive:**
- `public_id` UNIQUE index inserts are sequential — no B-tree page splits
- Rough chronological ordering is embedded in the ID, which aids debugging and log correlation
- One validation rule across all resource types: exactly 16 alphanumeric characters after the prefix separator
- No change to ID length, storage, or API surface

**Negative:**
- IDs are no longer purely opaque — a sophisticated client who knows the encoding can extract the creation-time millisecond from the first 8 characters. Clients must still treat `public_id` as an opaque token per the API contract, but this is a convention rather than a technical guarantee
- The random budget per millisecond drops from 62^16 to 62^8 compared to the original design; safe at 10 K/s but would need re-evaluation if throughput targets increase by 7 orders of magnitude
