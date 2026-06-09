# ADR api-design/009: Public ID length — 16-character NanoId on a 62-symbol alphabet

## Status
Accepted

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

The NanoId component of every `public_id` is **16 characters** drawn uniformly at random from the **62-symbol alphanumeric alphabet** (`0–9A–Za–z`) using a CSPRNG.

The full `public_id` is stored in a `VARCHAR` column sized to `len(prefix) + 1 + 16`:
- 3-char prefix (e.g., `ep_`, `dj_`): `VARCHAR(19)`, typically rounded to `VARCHAR(20)`
- 4-char prefix (e.g., `app_`, `evt_`): `VARCHAR(20)`

## Principles upheld

- **Frugality** — 16 characters is the minimum length that makes collisions negligible at any foreseeable scale; longer IDs add storage and wire overhead without a corresponding benefit
- **Security by design** — CSPRNG generation ensures IDs are not guessable or enumerable; 810 trillion distinct values renders brute-force enumeration computationally infeasible

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Length 12 | 67-billion n₅₀ poses real collision risk on `events` and `delivery_jobs` at large scale over time |
| Length 8 | 17-million n₅₀ reachable in days on active platforms — ruled out entirely |
| Length 21 (NanoId library default) | Provides ~1.2 × 10¹⁸ n₅₀ — 1 500× more margin than length 16 — for 5 extra characters per ID; the surplus margin buys nothing at any realistic scale |
| Variable length per resource type | Different validation rules for different resource types add operational complexity; a uniform 16-char rule is simpler and consistent |

## Consequences

**Positive:**
- One validation rule across all resource types: exactly 16 alphanumeric characters after the prefix separator
- Effectively zero collision probability at any foreseeable platform load or table cardinality
- 16-character IDs appear cleanly in logs, error messages, and UI without truncation

**Negative:**
- The collision guarantee is probabilistic, not absolute — it depends on the CSPRNG never producing correlated output; the `nanoid` crate delegates to the OS entropy pool, which is the industry-standard mitigation but not an absolute proof
- Clients cannot decode any information from the ID — there is no standard library for base62; clients must treat `public_id` values as opaque strings
