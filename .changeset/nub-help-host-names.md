---
"pacquet": patch
---

Under an embedder that does not read pnpm's configuration, `--help` names the profile's lockfile and settings file instead of `pnpm-lock.yaml` and `pnpm-workspace.yaml`, and a profile that declares workspaces in `package.json` gets `--ignore-workspace` and `--workspace-packages` described in those terms. A profile that only renames the lockfile gets its lockfile named. pnpm's own help is unchanged.
