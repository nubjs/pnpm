---
"pacquet": patch
---

Fix `store status` reporting every package as modified when the global virtual store is enabled. It built the project-flat slot path, which that layout does not use.
