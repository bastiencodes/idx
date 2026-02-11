---
tidx: patch
---

Hardened SQL query API: replaced string-based injection with AST manipulation, switched from function blocklist to allowlist, added table allowlist, enforced reject-by-default expression validation, capped LIMIT/depth/size, and locked down API role with connection and resource limits.
