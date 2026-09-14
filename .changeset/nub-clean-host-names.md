---
"pacquet": patch
---

`clean` removes the embedder profile's virtual store directory and the hidden entries a host lists in `Embedder::hidden_modules_dir_entries`, and `clean --lockfile` removes the profile's lockfile and its legacy names instead of `pnpm-lock.yaml`. Under a host with names of its own, `clean` no longer leaves `node_modules` or the lockfile behind.
