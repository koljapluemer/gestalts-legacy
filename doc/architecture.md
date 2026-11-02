# Architecture

```
crates/core        notes, gestalts, queries, 3-way merge; the Storage port. No IO.
crates/fs          Storage adapter: a folder on disk
crates/wasm        the core in the browser + adapters: OPFS, static site, connector (HTTP)
crates/connector   gestalts-connector: desktop app serving a folder on localhost, and the plugin host
gestalts/          gestalt definitions (data, embedded into the core)
packages/client    TS wrapper of the wasm core: Repo, React hooks, field helpers
packages/ui        primitives → components → widgets → perspectives → panes
apps/gestalter     main app: switch between all panes
apps/demo          public-facing Greek letters practice
```

## Core (Rust, native and wasm)

- `Storage` (port): list/read/write/delete notes by key, list/read/write blobs by name. `?Send` so browser adapters fit.
- Adapters: `FsStorage` (disk, atomic writes), `Opfs`, `Static` (read-only, a published data folder with `index.json`), `Connector` (HTTP), `Layered` (read-only base + writable top, copy-on-write), `Memory`.
- `Repo`: loads every note into an index (parsed YAML, gestalts, lints) and answers `Query {gestalt, tagsAll, tagsNone, keys}`.
- `merge3` / `sync`: see [connector.md](connector.md).
- `upstream`: copy-on-write from a published deck into the user's storage, see below.

The browser never parses YAML itself: `packages/client` calls the wasm core, which also lints drafts (`repo.inspect`) for the editor.

## Data flow in the apps

| app | storage | with connector |
|---|---|---|
| Gestalter | OPFS `gestalter/` | OPFS syncs with the folder |
| Demo | `public/gestalt-data` (read-only deck) layered under OPFS `demo/`; a word is copied into OPFS when first rated | only the OPFS layer syncs with the folder |

Both work without the connector. Connecting is remembered per browser.

### Copy-on-write from a deck

The Demo's own data must stand on its own: a folder synced from it holds the rated words, not just review states pointing at words that live elsewhere. So user space is **closed under references**:
- Writing a note through `Layered` copies every base note it references (`relationships`), with that note's blobs and, in turn, its references. Rating a word writes an `fsrs-state` note, which copies the word.
- `pull_upstream` runs on open and after each sync. It copies whatever is still missing (e.g. references that arrived by sync) and brings copies up to date.

Each copy is the upstream text plus an `upstream` trailer (see [data-format.md](data-format.md)). When upstream changes:
- an untouched copy is replaced with the new upstream text
- an edited copy is left alone and reported as diverged
- words nobody rated simply stay upstream, so they update for free

The repo's `upstreamReport()` says what the last check did; the Demo logs it to the console.

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
