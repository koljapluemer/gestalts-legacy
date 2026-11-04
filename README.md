# gestalts

Personal plain-text data (YAML notes + blobs in one folder) with interchangeable views on top.
See [`doc/`](doc/) — start with [`doc/architecture.md`](doc/architecture.md); the original idea is in [`doc/initial_spec.md`](doc/initial_spec.md).

## Developing

Needs [mise](https://mise.jdx.dev) and [rustup](https://rustup.rs). mise installs Node, pnpm, uv and the cargo tools
pinned in `mise.toml`; rustup picks up the Rust version from `rust-toolchain.toml`.

```sh
mise install          # once after cloning, and when mise.toml changes
mise run dev          # connector (dev profile, repo plugins) + Gestalter :5173 + Demo :5174, hot reload
mise run check        # tests, lints, typechecks
mise run check-plugin-standalone quotes   # build a plugin outside the workspace, as in its own repo
mise run publish-data plugins/greek-alphabet/deck     # after editing a plugin's deck
mise tasks            # everything else
```

On Linux, building the connector needs no GTK; its tray talks D-Bus directly.

## Shipping

- **Web frontends:** `mise run build` writes `apps/*/dist`, static files to host anywhere. Gestalter is meant for
  `https://gestalter.koljasam.com`, which the connector allows out of the box.
- **Connector:** `mise run package` builds this OS's installers into `target/packages` (Linux: `.deb`, `.rpm`,
  AppImage; macOS: `.dmg`; Windows: setup `.exe`). `mise run install` builds them and installs one here like a user
  would. The installed app needs nothing else: it brings its plugins, and downloads uv (and through it Python) for
  them when needed. See [`doc/connector.md`](doc/connector.md).
