---
"pacquet": patch
---

Commands that read the lockfile by name — `list`, `why`, `licenses`, `peers`, `outdated`, `store status`, `cat-index`, `deploy`, and the global update's downgrade check — now read the embedder profile's lockfile and its legacy names instead of `pnpm-lock.yaml`, and `list` and `why` find the current lockfile under the profile's virtual store directory. `deploy` writes the deployed lockfile under the profile's name, and the `sbom` stale-lockfile error names it. Under a host that renames its lockfile these commands no longer report an empty or clean tree.
