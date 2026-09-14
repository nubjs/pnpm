---
"pnpm": patch
---

`audit --fix` now names the file an embedding host actually records an override in, through the new `Embedder::overrides_file_display_name`. A host that supplies an `overrides_writer` keeps overrides outside pnpm's workspace manifest, and substituting its settings file left the flag describing a file the fix never writes.
