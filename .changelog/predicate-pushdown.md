---
tidx: minor
---

Add predicate pushdown for indexed event parameters.

- Rewrites SQL filters like `"from" = '0x...'` to `topic1 = '0x000...'` to enable index usage
- Add `signature` parameter to `/views` API for automatic CTE generation and decoding
- Support both PostgreSQL and ClickHouse query engines
