# Architecture

```
crates/core        notes, gestalts, queries, 3-way merge; the Storage port. No IO.
crates/fs          Storage adapter: a folder on disk
crates/wasm        the core in the browser + adapters: OPFS, static site, connector (HTTP)
crates/connector   gestalts-connector: desktop app serving a folder on localhost, and the plugin host
gestalts/          gestalt definitions (data, embedded into the core)
packages/client    TS wrapper of the wasm core: Repo, React hooks, field helpers, the learning layer (FSRS, sessions), the debug log
packages/ui        primitives → components → widgets → perspectives → panes
apps/gestalter     main app: switch between all panes
apps/demo          public-facing Greek letters practice
```

The planned changes to merging, deletes and plugins are in [foundations.md](foundations.md).

## Core (Rust, native and wasm)

- `Storage` (port): list/read/write/delete notes by key, list/read/write blobs by name. `?Send` so browser adapters fit.
- Adapters: `FsStorage` (disk, atomic writes), `Opfs`, `Static` (read-only, a published data folder with `index.json`), `Connector` (HTTP), `Layered` (read-only base + writable top, copy-on-write), `Memory`.
- `Repo`: loads every note into an index (parsed YAML, gestalts, lints) and answers `Query {gestalt, gestaltsNone, tagsAll, tagsNone, keys, where}`.
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

- `primitives/`: CodeMirror host and theme, `HighlightedText`, text folding, `Markdown` (math, `[[id|label]]` links, `![[blob]]` embeds: images, video, audio, else a link), `MarkdownEditor` (`[[` completion, references openable with Mod-click)
- `components/`: `NoteLine`, `GestaltBadges`, `YamlEditor`
- `widgets/`: `NoteResultList` (with per-result actions), `NoteView`, `NoteTextEditor`, `EditableNote` (YAML), `NoteParts` (cards and template editors), `Relationships` (with backlinks), `MediaGallery`, `NotePicker`, `SourcePicker`, the note search (`noteSearch.ts`)
- `perspectives/<slice>/`: `search-sidebar`, `universal-editor`, `new-note`, `greek-alphabet`, `learn`, `connector-settings`, `canvas`, `typst-editor`, `sources`. Slices don't import each other, except the shared `define` slice.
- `panes/`: `ExplorerPane` (search + editor), `NewNotePane` (the new-note form on its own), `CanvasPane`, `Canvas2TypPane` (a note's canvas beside its Typst document), `SourcesPane`, `LearnPane`, `GreekAlphabetPane`. They share `SearchLayout`: the search on the right, whose results offer the pane's actions (open as canvas, add to this canvas, …), and at most one overlay over the pane (`panes/overlay.ts`): a note in the editor, or the new-note form the search's "+" opens. The overlay lives in the URL (`?overlay=note:<key>` or `new`); opening one pushes a history entry, so Back closes it, and opening another from inside it replaces it. Panes import perspectives only, through `perspectives/index.ts`.

- `debug/` (`@gestalts/ui/debug`, beside the layers, for app shells): `ErrorBoundary` (around each pane), `DebugLogButton`. See [debugging.md](debugging.md).

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

Optional event props are capabilities: if the host leaves one unwired, the perspective hides the UI for it (no "+" without `onCreate`, no open buttons without `onOpen`). A perspective never knows how it is presented, inline or in a modal. Nothing is written to storage just to show something: the new-note form writes the note only on Create.

## Canvas and Typst

A canvas is the `canvas` key of its host note (`canvas@2`); its linked notes are `on-canvas` relationships of the
host, so they travel with it. An old `canvas@1` note (`canvas-of` the host) is shown until the first change folds it
into the host and retires it ([gestalts.md](gestalts.md#folded-parts)). While it's open, the page's reducer is the
only truth (`perspectives/canvas/model.ts`); `CanvasSaver` writes whole snapshots, debounced, one at a time, held
while dragging. If the stored nodes change to something this page didn't write (a sync, the YAML editor), it stops
writing and offers to reload; changes to the host's other keys don't count. A host whose YAML doesn't parse makes
the canvas read-only.

The note editor shows a note's cards and template below its YAML (`widgets/NoteParts.tsx`), folding attached ones in
on the first save the same way. Since a note can now be written by these and by the YAML editor at once, the YAML
editor saves a draft as a 3-way merge against the text it started from (the core's `merge3`, per top-level key). If
both changed the same key, it asks before overwriting.

The Typst editor compiles through the connector's `typst@1` service (`plugins/typst`), sending the source, the
document's `assets` (path → blob, fetched by the worker) and a bibliography of the sources it cites
(`/sources.bib`). Without the service the document is still editable and shows its last built PDF. Build PDF stores
the PDF as a content-addressed blob labelled `pdf`; saving adds a `cites` relationship to each source cited.

## Note view

The editor (`universal-editor`) shows a note in one of three modes, remembered per note for the browser session:
- **Read** (`widgets/NoteView.tsx`): its images and videos (blobs labelled `image`/`video`, or unlabelled ones that
  are by extension) in a gallery, then `tasks`, `main`, `extra` and its cards rendered, then its other blobs as links.
- **Text** (`widgets/NoteTextEditor.tsx`): title, `main` and `extra` as Markdown, with a preview beside them (below
  when narrow). `[[` completes a note from the search and inserts its id. Mod-click on a reference opens it, and on
  touch screens a button above the reference under the cursor does. Saves on blur and Mod-s, field by field against
  the note as it is by then. If the note changed one of the same fields meanwhile, it asks.
- **YAML**: the raw note, as before.

Images and videos are attached from the toolbar (on touch screens also from the camera), or by pasting or dropping
them into the text. They're written at once, as content-addressed blobs. Images are downscaled to 1920 px and
re-encoded as WebP (lossless from PNG and BMP; GIF and SVG kept as they are). Videos up to 250 MB are stored as they
are. Removing one takes it off `blobs`; the blob itself stays.

Every save of the text, of the YAML or of the cards keeps the note's `inline` relationships in step with the
`[[references]]` in `main`, `extra` and its cards, and lists each `![[embed]]` in `blobs` (`syncInline`,
`packages/client/src/inline.ts`). The copy-on-write closure and backlinks then follow prose links like any other
relationship. A YAML save is written as typed unless that changes something.

`Relationships` lists a note's own relationships (`→`), its inline references (`⇢`) and its backlinks (`←`, `⇠`).
The backlinks come from one index per repo version, built from every note's relationships and from the references
in their text, so notes edited elsewhere count too. Explicit rows can be detached and relabelled, and the adder
relates a picked note in either direction. Each change is written at once, to whichever note holds the relationship.
Inline rows are edited in the text.

## Learn

The Learn pane (`perspectives/learn`) reviews every flashcard by FSRS, as dsl's Learn tab did. The cards come from
`learnableCards` (`packages/client/src/learn.ts`): each card in a note's `cards`, each `flashcard@1` that is
`card-of` a note without `cards` yet, and each `flashcard@1` of its own. A card of a note is reviewed as the facet
`<card id>` of that note, so its state stays put when the card is folded in; a flashcard of its own as the whole note.
`pickNext` shows a random due card, else a random new one, never the one just rated unless it's the only one; the
card shown stays until it's rated, even if a sync changes the picks. The front, then Reveal (Space), then front and
back with Again, Hard, Good, Easy (1–4); the counts are due · new · all. Reviews go through `saveReview` and the
`learn` practice session. A rating shows the next card at once: `useLocalReviews` lays the new state over the
query's until the query has it, and the save runs in the background (it may wait behind a sync); if it fails, an
alert says so. The Greek alphabet works the same way. A card that's deleted leaves its `fsrs-state` note behind: nothing reads it, and deleting
it would lose against sync, where an edit beats a delete.

## Search sidebar

Search runs client side over every note:
- Every query word must match a word of the note, or a prefix of one. Case and accents are ignored.
- Rarer words weigh more, and exact words beat prefixes.
- A match in `title`, `aliases` or `main` beats a match elsewhere.
- Filters: `#tag` or `tag:x`, `gestalt:name@1`, `-word`.
- An empty query lists every note by label. The storage port has no modification times, so there's no "recent" list.

Keys: `/` focuses the search. Use ↑/↓ and Enter to open a result, and Esc to leave the field.
