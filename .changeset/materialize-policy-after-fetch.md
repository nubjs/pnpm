---
"pacquet": patch
---

A host's `MaterializePolicy` is now asked again once the install has extracted the packages it fetched, and every slot is placed by that second answer. The first answer is taken while the layout is built, before a byte is fetched, so a policy that decides by what a package CONTAINS could only ever answer for packages the store already held — on the install that first fetches a package, the one where being kept out of the shared store matters most, it had nothing to read. The install now downloads its cold batch before it links any slot, flushes the store index so the policy can read back the rows this run wrote, and takes the answer again; the second answer only ever adds to the first.

That ordering costs the overlap between the link pass and the downloads: under a policy the warm slots are linked after the cold batch has landed rather than while it is still downloading, so an install with something cold pays the link time at the tail instead of hiding it under network latency. A warm importer of a package the second answer keeps local has to be kept local too, so linking it earlier would point its child symlink at a shared slot nothing goes on to create — the serialization is what makes the answer sound, and it is deliberate. pnpm sets no policy, so its install order and its store-index writer are untouched.
