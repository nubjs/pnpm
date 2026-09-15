---
"pacquet": patch
---

A host's `MaterializePolicy` is now asked again once the install has extracted the packages it fetched, and every slot is placed by that second answer. The first answer is taken while the layout is built, before a byte is fetched, so a policy that decides by what a package CONTAINS could only ever answer for packages the store already held — on the install that first fetches a package, the one where being kept out of the shared store matters most, it had nothing to read. The install now downloads the cold batch before it links any slot, flushes the store index so the policy can read back the rows this run wrote, and takes the answer again; the second answer only ever adds to the first.

That ordering costs the overlap between the link pass and the downloads: on the second-answer path the warm slots are linked after the cold batch has landed rather than while it is still downloading, so a partly-cold install pays the link time at the tail instead of hiding it under network latency. It is the price of a sound answer and it is deliberate. Only a host with a policy pays it, and only on an install with something cold: pnpm sets no policy, and a fully warm install has no cold batch, so both keep the original order.
