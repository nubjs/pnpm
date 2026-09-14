---
"pacquet": patch
---

A host embedding the engine can now read `npm_config_registry`, the proxy variables and the TLS variables such as `npm_config_strict_ssl` and `npm_config_cafile`. They rank above the project `.npmrc`, as npm ranks them. pnpm itself still reads none of them.
