---
"pacquet": patch
---

`pnpm patch-commit` and `pnpm patch-remove` can now record `patchedDependencies` through a host that supplies its own writer, instead of refusing. A host embedding the engine keeps the entry in its own configuration, so the workspace manifest is left untouched. A host with no writer is refused before the package is fetched, and the `pnpm patch` hint names the running program.
