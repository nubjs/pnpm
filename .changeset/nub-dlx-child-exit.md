---
"pacquet": patch
---

A `dlx` child that exits nonzero no longer has to end the process. A host embedding the engine can ask for the failure as `ERR_PNPM_DLX_CHILD_FAILED`, carrying the child's exit code, which keeps it distinguishable from a failure to fetch the tool.
