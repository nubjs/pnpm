---
"pacquet": patch
---

`pnpm link <dir>` can now record its overrides through a host that supplies its own writer, instead of refusing. A host embedding the engine keeps its overrides in its own configuration, so the workspace manifest is left untouched.
