dir := justfile_directory()

# install JS dependencies and build the core for the browser (once, and after Rust changes)
setup: wasm
    pnpm install

# compile the Rust core to wasm for the web apps
wasm:
    cd {{dir}} && wasm-pack --log-level warn build crates/wasm --target web --out-dir ../../packages/client/wasm --out-name gestalts

# Gestalter with hot reload on :5173
gestalter: wasm
    pnpm --dir {{dir}}/apps/gestalter dev

# the Greek letters demo with hot reload on :5174
demo: wasm
    pnpm --dir {{dir}}/apps/demo dev

# serve a data folder (notes/ + blobs/) to the apps on :7717
connector DATA:
    cargo run --release -q -p gestalts-connector -- --data {{DATA}} --plugins-dir {{dir}}/plugins

# print each note's gestalts and lints for a data folder
lint-data DATA:
    cargo run --release -q -p gestalts-connector -- --data {{DATA}} --lint

# prepare a data folder for publishing as a read-only deck: content-addressed blobs, index.json, lints
publish-data DATA:
    cargo run --release -q -p gestalts-connector -- --data {{DATA}} --publish

# test, lint (incl. layer rules) and typecheck everything
check: wasm
    cargo test --workspace -q
    cargo clippy --workspace -q -- -D warnings
    pnpm --dir {{dir}} lint
    pnpm --dir {{dir}} typecheck
    pnpm --dir {{dir}} test

# production builds of both apps into apps/*/dist
build: wasm
    pnpm --dir {{dir}} -r --filter "./apps/*" build
