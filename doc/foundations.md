# Foundations: merge, deletes, plugins

The design the pre-split steps in [future.md](future.md#plans) build toward. Not built yet. As each part lands, its
details move into the docs it belongs to ([connector.md](connector.md), [gestalts.md](gestalts.md),
[data-format.md](data-format.md), [architecture.md](architecture.md)) and this file keeps the overview.

## Merge rules per gestalt

Sync merges each note against the base, the state after the last sync. Many conflicts have an obvious answer in the
domain ("the latest review wins"). That answer belongs to the gestalt, not to a perspective: sync runs without any
perspective open (and later in workers, on Android), and two perspectives writing the same gestalt must agree.

A gestalt may declare how its notes merge:

```yaml
# fsrs-state/v1.yml: pick one side as a whole
merge:
  whole: { newest_by: [card.lastReviewAt, card.reps] }
```
```yaml
# practice-session/v1.yml: per key
merge:
  keys:
    events: { union_by: at }
    active_ms: max
    ended: max
```

Strategies, each a pure function of `(base, local, remote)`:

| strategy | |
|---|---|
| `conflict` | the default: a key changed differently on both sides is a conflict |
| `map` | merge per key, recursing |
| `set` | a list as a set: additions from either side kept, removals from either side dropped |
| `union_by: <field>` | a list of items identified by a field, all of them kept (append-only logs) |
| `list_by: <field>` | a list of items identified by a field, each merged in turn (`cards` by `id`, canvas `nodes` by `id`) |
| `text` | line-level diff3; a conflict only where both changed the same lines |
| `newest_by: [paths]` | the side with the greater value at the first path that differs; works without a base |
| `max`, `min` | the greater or smaller value |

Every note has a default policy: `tags`, `relationships` and `blobs` merge as sets; `main`, `extra` and `tasks` as
text; everything else per key. A note's policy is the default plus the policies of every gestalt the note has on any
of the three sides. Where they disagree on a key, it falls back to `conflict`.

The engine is its own crate, `crates/merge`, which knows nothing of notes or storage. It is **symmetric**:
`merge(b, l, r) == merge(b, r, l)`. A conflict picks a winner by a deterministic tiebreak, never by "local", so any
two devices merging the same states get the same result. Its laws (identity, idempotence, symmetry, no silent loss,
and one per strategy) are property-tested.

`newest_by` trusts device clocks, and two offline reviews of one card keep only the later one. For learner state
that's the right trade.

## Conflicts

A conflict that no rule settles is a `conflict@1` note: `conflict-of` the note, the key path, and the base, local
and remote values. It's keyed by note and path, so devices recording the same conflict write one note. The note
itself keeps the winning value.

- The note editor and the connector settings offer a generic resolver: pick a side, or edit.
- A perspective that understands a gestalt may resolve its conflicts in code when it reads them. That's safe if the
  resolution is deterministic: devices resolving the same conflict write identical text, which merges cleanly.

## Tombstones

Deleting a note writes it as a tombstone, `deleted: <datetime>`, kept forever. The index and queries skip
tombstones.

- A delete against an unchanged note deletes it.
- A delete against a concurrent edit also deletes it, and the edited text is kept as a `conflict@1` record, so it can
  be restored. Deleted panes, plugins and cards stay deleted.
- Writing a note over a tombstone brings it back.
- A file removed by hand from the folder still counts as a delete, through the base.
- In `Layered`, a tombstone in the top layer hides the deck note under it (a whiteout), and copy-on-write never
  copies a tombstoned key back.
- A relationship may die with its target: `reviews: { min: 1, max: 1, dies_with_target: true }`. Deleting a card
  tombstones its learner state.

`retired` stays what it is: a deck saying a note is obsolete. A tombstone is the storage-level delete.

## Blobs

Blobs are content-addressed, so they're never edited and never tombstoned: a tombstone would block the same content
from being attached again. They're collected instead:

- A blob is live while a note that isn't a tombstone references it, in any layer, or a conflict record does.
- Every reference is listed in `blobs` (a lint checks it).
- Sync copies a blob to the other side only if a merged note references it, so collecting on one side isn't undone
  by the other.
- Each writable storage collects after a successful sync. A manifest records when each blob was first seen
  unreferenced, and it's deleted only after staying unreferenced for some days and some syncs.
- Decks are never collected.

## Perspectives and panes

A **perspective** is the unit of the contract and of reuse. What goes in:
- its declared queries, kept live
- its inputs
- blob URLs
- services

What comes out:
- writes, checked against its declared `access` and linted against the gestalts before they're accepted
- events
- requests for **host capabilities**: open a note, pick a note, navigate, set the title

Notes cross the port as parsed, gestalt-shaped values. Only the core parses YAML.

A **pane** is the unit of installing, of opening and closing, and of isolation. A plugin's pane composes the
plugin's own perspectives. Plugins compose with each other through notes and host capabilities, never through each
other's props.

**Slots** are places in first-party panes with a narrow interface owned by the host, which a plugin may fill with a
perspective: a view of gestalt X in the note editor, a canvas node, a card in Learn.

The port is published as `@gestalts/sdk` (`definePerspective`, `definePane`, hooks, host capabilities).
`@gestalts/kit` holds the shared components that are deliberately public. First-party perspectives use the same
scoped port, so moving one into a sandbox is a deployment change, not a rewrite.

## Plugins

A plugin is a domain bundle:

```
plugins/<id>/
  plugin.yml
  gestalts/<name>/v1.yml       # gestalts this plugin owns
  deck/                        # optional starter data, published-deck format
  ui/                          # optional: one ESM bundle, built against @gestalts/sdk and @gestalts/kit only
  worker/                      # optional: as in connector.md
```

```yaml
schema: 2
id: greek-alphabet
version: 1.0.0
host_api: 1
gestalts: [pronunciation-card@1]           # owned here
uses: [fsrs-state@1, practice-session@1]   # owned elsewhere: a data dependency
deck: deck/
panes:
  - { id: practice, title: Greek alphabet, entry: ui/dist/index.js#PracticePane }
slots:
  - { slot: note-view, gestalt: pronunciation-card@1, entry: ui/dist/index.js#CardView }
access:
  read:  { gestalts: [pronunciation-card@1, fsrs-state@1, practice-session@1] }
  write: { gestalts: [fsrs-state@1, practice-session@1] }
```

- **Gestalts:** each one has exactly one owner, the core or one plugin. The host loads a plugin's gestalts at
  runtime, so their merge rules apply in sync.
- **Code:** a plugin never imports another plugin. Shared logic lives in libraries such as `@gestalts/learning`
  (FSRS, sessions).
- **Data:** a plugin's deck is mounted read-only under the user's folder. `Layered` takes any number of decks, each
  under its own `source` id. A deck may declare aliases for that id, so existing copies keep following it.
- **Uninstalling** unmounts the deck. The user's copies stay, because user space is closed under references.
  Removing the user's data from a plugin as well is a separate, explicit choice: it tombstones the notes whose
  `upstream.source` is that plugin.
- **Loading:** the host loads plugins and never imports them. Built-in plugins are bundled into an app, but they're
  found through a generated registry, so the app builds without them.

Trust tiers:

| tier | from | runs |
|---|---|---|
| built-in | an app's distribution | in-process |
| local | `plugins/` in the data folder, or a perspective note | sandboxed by default, or in-process if marked trusted |
| third-party | a URL or registry | sandboxed; installing shows `access`; a worker needs approval |

The sandbox (an iframe per pane) comes after the split ([future.md](future.md#later)). Until then, only built-in
plugins run.

## Distributions

An app is the host plus a list of plugins:

```yaml
# apps/demo/distribution.yml
plugins: [greek-alphabet]
default_panes: [greek-alphabet/practice]
install_ui: false
```

## Installs and panes are notes

They live in the user's folder, so they sync and merge like any other note:

- `plugin-install@1`, keyed `stableId("plugin", id)`: `{plugin, source, version, enabled}`. Uninstalling tombstones
  it.
- `pane@1`: `{pane: <plugin>/<pane>, title, params, order}`. Opening a pane creates a note, and closing it tombstones
  the note. `order` is fractional and merges by `newest_by`. A pane's own settings live on its note.

A folder without pane notes is seeded with the distribution's `default_panes`, using stable keys, so two devices
seeding at once write the same notes.

## Repos

One repo for now: the host (core, merge, wasm, client, ui, connector, apps) and its contracts. Plugins live in
`plugins/<id>/` as if they were external: a lint allows them only `@gestalts/sdk`, `@gestalts/kit` and
`@gestalts/learning`. A plugin moves to a repo of its own once the contract holds for one that was ported that way
(`quotes`).
