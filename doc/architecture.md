# Architecture

```
crates/core        notes, gestalts, queries, 3-way merge; the Storage port. No IO.
crates/fs          Storage adapter: a folder on disk
crates/wasm        the core in the browser + adapters: OPFS, static site, connector (HTTP)
crates/connector   gestalts-connector: serves a folder on localhost
gestalts/          gestalt definitions (data, embedded into the core)
packages/client    TS wrapper of the wasm core: Repo, React hooks, field helpers
packages/ui        primitives → components → widgets → perspectives → panes
apps/gestalter     main app: switch between all panes
apps/demo          public-facing Greek letters practice
```

## Core (Rust, native and wasm)

- `Storage` (port): list/read/write/delete notes by key, list/read/write blobs by name. `?Send` so browser adapters fit.
- Adapters: `FsStorage` (disk, atomic writes), `Opfs`, `Static` (read-only, a published data folder with `index.json`), `Connector` (HTTP), `Layered` (read-only base + writable top), `Memory`.
- `Repo`: loads every note into an index (parsed YAML, gestalts, lints) and answers `Query {gestalt, tagsAll, tagsNone, keys}`.
- `merge3` / `sync`: see [connector.md](connector.md).

The browser never parses YAML itself: `packages/client` calls the wasm core, which also lints drafts (`repo.inspect`) for the editor.

## Data flow in the apps

| app | storage | with connector |
|---|---|---|
| Gestalter | OPFS `gestalter/` | OPFS syncs with the folder |
| Demo | `public/gestalt-data` (read-only) layered under OPFS `demo/` | the OPFS layer syncs with the folder |

Both work without the connector. Connecting is remembered per browser.

## UI layers (`packages/ui/src`)

Each layer imports only from layers below. All may use `@gestalts/client` and external packages.
Tailwind + daisyUI + lucide are the real primitives; `primitives/` only adds what they lack.

- `primitives/`: CodeMirror host and theme, `HighlightedText`, text folding
- `components/`: `NoteLine`, `GestaltBadges`, `YamlEditor`
- `widgets/`: `NoteResultList`, `EditableNote`
- `perspectives/<slice>/`: `search-sidebar`, `universal-editor`, `greek-alphabet`, `connector-settings`. Slices don't import each other, except the shared `define` slice.
- `panes/`: `ExplorerPane` (search + editor), `GreekAlphabetPane`. Panes import perspectives only, through `perspectives/index.ts`.

`oxlint.config.ts` enforces this (eslint-plugin-boundaries). Within primitives, components and widgets, files of the same layer may share code. "Not from neighbours" is enforced between perspective slices only.

## Perspectives

```ts
export const GreekAlphabet = definePerspective<{ cards: Query; reviews: Query }, Props>({
  id: 'greek-alphabet',
  title: 'Greek alphabet',
  data: ({ deckTag = 'greek-alphabet' }) => ({
    cards: { gestalt: 'pronunciation-card@1', tagsAll: [deckTag] },
    reviews: { gestalt: 'fsrs-state@1' },
  }),
  Component: ({ data, repo, ...props }) => ...,
});
```

A perspective's ports:
- `data`: named queries it reads, resolved to notes and kept up to date.
- `repo`: writes and blobs.
- Its own props: inputs and events.

The pane that hosts it wires those props. For example, `ExplorerPane` passes `SearchSidebar`'s `onOpen` into `UniversalEditor`'s `open` list, which it keeps in the URL. That is how panes orchestrate events between perspectives. There is no global event bus.

## Search sidebar

Search runs client side over every note:
- Every query word must match a word of the note, or a prefix of one. Case and accents are ignored.
- Rarer words weigh more, and exact words beat prefixes.
- A match in `title`, `aliases` or `main` beats a match elsewhere.
- Filters: `#tag` or `tag:x`, `gestalt:name@1`, `-word`.
- An empty query lists every note by label. The storage port has no modification times, so there's no "recent" list.

Keys: `/` focuses the search. Use ↑/↓ and Enter to open a result, and Esc to leave the field.
