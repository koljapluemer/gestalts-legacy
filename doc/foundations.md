# Foundations: merge, deletes, plugins

What the repo was made ready with for splitting into repos (the pre-split steps in [future.md](future.md#plans)).
It's all built; the details live in the docs they belong to, and this file keeps the overview and the trade-offs.

## Merge, conflicts, tombstones

Sync merges each note by rules its gestalts declare, with a symmetric engine (`crates/merge`), so it needs no
perspective and any two devices agree. What no rule settles is a `conflict@1` note, resolved in the editor or
deterministically in code. Deleting writes a tombstone, which wins over a concurrent edit (the edit kept as a
conflict), hides deck notes in `Layered`, and takes `dies_with_target` notes along. Details:
[gestalts.md](gestalts.md#merge), [connector.md](connector.md#sync), [data-format.md](data-format.md).

- A conflict record names its sides `kept` and `lost`, not "local" and "remote": two devices recording the same
  conflict write the same text. Its key includes a hash of the values, so a new conflict at the same path doesn't
  overwrite an unresolved one.
- `newest_by` trusts device clocks, and two offline reviews of one card keep only the later one. For learner state
  that's the right trade.
- `retired` stays what it is: a deck saying a note is obsolete. A tombstone is the storage-level delete.
- Conflict copies from before (`<key>.conflict-<timestamp>`) stay ordinary notes.

## Blobs

Blobs are content-addressed: never edited, never tombstoned, collected instead once nothing references them for
some days and syncs. Sync copies only referenced blobs, so collecting on one side isn't undone by the other. Every
reference is listed in `blobs` (a lint checks embeds and fields of type `blob`); decks are never collected.

## Perspectives, panes, plugins

[architecture.md](architecture.md#packages-and-plugins) has how they work. In short:

- A **perspective** is the unit of the contract and of reuse; `@gestalts/sdk` is its port. A **pane** is the unit
  of installing, opening and closing; it composes its plugin's perspectives. Plugins compose with each other only
  through notes, host capabilities and slots.
- A **plugin** is a domain bundle (`plugins/<id>/`): the gestalts it owns, a deck, UI, a worker. Every pane is in a
  plugin, the first-party ones too. The host keeps only its chrome: the note editor and new-note form (as host
  capabilities), connector settings, the plugin menu, the conflict resolver.
- Each gestalt has one owner. Those more than one plugin uses are the core's, so uninstalling a plugin never takes a
  shared gestalt's rules away.
- An **app** is the host plus a distribution. Installs and panes are notes, so they sync and merge.
- Canvas2Typ (typst plugin) shows a note's canvas through the canvas plugin's `note-view` slot. A slot's interface is
  narrow (the note), so adding a note to the canvas from Canvas2Typ's search is gone: that's the Canvas pane's.

## Trust tiers

| tier | from | runs |
|---|---|---|
| built-in | an app's distribution | in-process, through the same scoped port as any plugin |
| local | `plugins/` in the data folder, or a perspective note | sandboxed by default, or in-process if marked trusted |
| third-party | a URL or registry | sandboxed; installing shows `access`; a worker needs approval |

Only built-in plugins exist so far. The sandbox (an iframe per pane) comes after the split
([future.md](future.md#later)); since first-party code already uses the scoped port, moving a plugin into one is a
deployment change.

## Repos

One repo for now: the host (core, merge, wasm, client, ui, connector, apps), its contracts (sdk, kit, learning) and
the plugins, which live in `plugins/<id>/` as if they were external: the lint allows them only `@gestalts/sdk`,
`@gestalts/kit` and `@gestalts/learning`, and `mise run check-plugin-standalone <id>` builds one outside the
workspace against the packed public packages. `quotes` builds that way, so it can move to a repo of its own.
