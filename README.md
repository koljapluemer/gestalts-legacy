# gestalts

Personal plain-text data (YAML notes + blobs in one folder) with interchangeable views on top.
See [`doc/`](doc/) — start with [`doc/architecture.md`](doc/architecture.md); the original idea is in [`doc/initial_spec.md`](doc/initial_spec.md).

Needs Rust (with the `wasm32-unknown-unknown` target), `wasm-pack`, Node ≥ 24, pnpm and `just`.

```sh
just setup            # once
just gestalter        # http://localhost:5173
just demo             # http://localhost:5174
just connector ~/my-gestalt-data   # optional: a folder on disk as source of truth
just check
```
