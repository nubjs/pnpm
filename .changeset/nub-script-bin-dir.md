---
"pacquet": patch
---

A host embedding the engine can contribute a directory of its own executables to every script the engine spawns, without putting it on the process `PATH`. It lands immediately ahead of the inherited `PATH`, so a host that used to prepend the directory itself resolves commands in the same order while the environment the engine reads stays as it found it — which matters because the build pipeline's Cargo cache hashes `PATH`, so a directory named per process would move the key on every run. Standalone pnpm supplies none and its script `PATH` is unchanged.
