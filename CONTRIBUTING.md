# Contributing

Changes to wire bytes, admission order, error precedence, key-codec laws,
checkpoint semantics, digest inputs, or resource accounting require an
invariant-ledger update before production implementation.

Run the complete local gate:

```sh
make verify
```

Do not weaken, rename, or delete a mapped property to make a change pass. Add a
causal mutant for each new invariant, preserve constant native stack use, and
capture validation evidence below the ignored `target/` directory. Keep
filesystem and runtime artifact policy outside this crate.
