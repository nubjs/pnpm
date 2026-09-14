---
"pacquet": patch
---

A `dlx` child that exits nonzero no longer has to end the process. A host embedding the engine can ask for the failure as `ERR_PNPM_DLX_CHILD_FAILED` and read the child's exit code back with `dlx_child_exit_code`, which keeps a tool that ran and failed distinguishable from a failure to fetch the tool at all.
