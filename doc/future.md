# Not built yet

The gestalts planned for bringing in dsl and sam-learns-things: [integration-gestalts.md](integration-gestalts.md).

## Fewer attached notes

A separate note is for something with its own writer or identity. A note's own canvas, template, cards or renders are
parts of that note and should be keys on it. Merge is per top-level key, so a `canvas` key never conflicts with `main`,
and `relationships`, `blobs` and `tags` merge as sets ([connector.md](connector.md)), so a canvas save adding an
`on-canvas` relationship no longer conflicts with a relationship edited on another device.

1. **Fold into the host**, each as a new gestalt version found by its key (`trigger_fields`), so old notes stay readable:
   - `canvas@2`: a `canvas: {nodes}` key; `on-canvas` relationships on the host.
   - `template@2`: a `template: {format, source}` key on the note it renders (`_index`, the portfolio).
   - `cards`: a list of `{front, back}` on the host, replacing `flashcard@1` notes with `card-of`. A card's review
     state is an `fsrs-state` with `facet` = the card's id (hash of its front, as dsl's `card_id`). A note that is just
     a card (just-flashcards) stays `flashcard@1`.
   - Worker output (card collages, viz builds) as keys the worker owns: `renders`, `viz`. Their blobs are also listed
     in `blobs`, labelled, so copy-on-write carries them.
2. **Migrate in place, lazily.** The dsl migration has run, and its `canvas@1`, `template@1` and `card-of`
   `flashcard@1` notes are in use and syncing between devices. There's no bulk rewrite. Perspectives read both forms,
   the host's key first. The first write through the new form folds the attached note into the host and tags the
   old note `retired`. It isn't deleted, since sync lets an edit beat a delete. If another device edits the old note
   after that, it's newer than the fold: offer it as a conflict on the host, as the canvas's reload does today.
   New notes are written in the new form only.

Separate notes stay where they have their own writer or identity:
- learner state on deck content (`fsrs-state`, `practice-state`): it must not touch the copied note, or the copy stops
  being untouched and no longer fast-forwards ([copy-on-write](architecture.md#copy-on-write-from-a-deck))
- `practice-session`: append-only logs, one writer each
- `source@1`, cited by many notes; `typst-document@1`, an essay of its own and possibly several per note

## Plans

One plan session each, in this order. Each ends with a visual check in the app and `doc/` updated.

Foundations:
- [x] Set merge for `relationships`, `blobs`, `tags` in `merge3`
- [ ] `Query.gestaltsNone`; the gestalts of [integration-gestalts.md](integration-gestalts.md), `fsrs-state` `facet`
- [ ] Lazy fold-in: `canvas@2`, `template@2`, `cards`, retiring the attached notes
- [ ] A repo per deck, so big decks don't load into one index
- [ ] Shared learning layer: FSRS by facet, `practice-session` writer (trials, active time), cross-app stats page
- [ ] Note parity with dsl: rendered `main`/`extra` with `[[links]]`, embeds, math and cards; backlinks widget;
  media attach and camera; `[[` completion

dsl:
- [ ] Learn
- [ ] Daily and Calendar (day/range views, past-appointment cleanup)
- [ ] Projects, Goals, Tasks
- [ ] Desk, Explorations, Tags, Suggest
- [ ] Queue, Float, Questions (Ollama service)
- [ ] Graph
- [ ] Published and the site export
- [ ] Portfolio, card collages (`renders`, Chromium plugin), PDF export
- [ ] Viz blocks (`viz`, claude CLI plugin)
- [ ] Semantic search (Ollama embeddings), continuous `.bib` export, orphaned-blob cleanup

sam-learns (each: catalog → deck, its Dexie tables → notes (do not migrate the actual per-user learning data, not needed)):
- [ ] quotes
- [ ] learn-flags, world-map
- [ ] top-vocab, dont-translate
- [ ] 20-words
- [ ] with-videos
- [ ] infinitesentences
- [ ] saetze, egyptiansentences
- [ ] hebrewscript, minimal-pairs-practice
- [ ] arabicnumbers, typingpractice
- [ ] currency-conversion-practice, triangle-congruence
- [ ] simplify-expressions
- [ ] tprboard
- [ ] al-kutshina

## Other

- **Android connector**: the same `Storage` port over Android storage. The core has no desktop-only dependencies.
- **Obsidian two-way sync**: likely a converter between Markdown with frontmatter and notes, plus the existing `sync`.
- **Browser plugin** (e.g. vocabulary from Duolingo): it would write through the connector API.
- Modification times in the `Storage` port, which the search would need for a "recent" list.
- Authentication for the connector (a token per app).
- A Learn pane for flashcards, `flashcard@1` notes and `cards` on notes (the ones migrated from dsl have no review state yet; `fsrs-state@1` is ready for it).
- Rendering `main` as Markdown in the Explorer, with `[[id]]` links and `![[blob]]` embeds (the canvas does already).
- Continuous `.bib` export to a path on disk, from the connector (the Sources pane downloads one).
- Undo for more than the last canvas removal; text ↔ note conversions are not undoable yet.
- Learner activity and time statistics. The original sam-learns app had these; they weren't migrated.
- Decks, beyond copy-on-write ([architecture.md](architecture.md#copy-on-write-from-a-deck)):
  - a "download the whole deck" action that copies everything up front, for offline use or archiving
  - a 3-way merge for diverged copies. It needs the deck text as copied, so either keep old deck releases or store a base snapshot next to the copies.
  - a deck-subscription note in user space (source URL, revision, words the user dismissed), so the folder records where the rest of the deck comes from
  - a UI for `upstreamReport()` ("3 words updated")
  - blob garbage collection: after a copy is fast-forwarded to a new image, the old blob stays in user space, because blobs are never deleted, here or in sync

## Plugins and backend workers

How perspectives with a heavy backend (e.g. a Python pipeline like `../news` or `../language-sketchbook-to-anki`) could fit in. Nothing of this is built. Start by porting one backend by hand, and extract the contract once there are two.

**A worker is another client of the data, not part of a perspective.** It reads notes and writes notes and blobs. The folder is how it talks to the UI:
- Batch work (fetch, OCR, LLM calls) runs on notes: an `interest-profile` note goes in, `news-item` notes come out; a `sketchbook-page` note with `status: queued` goes in, crops and proposals come out. Reviewing is editing notes, and committing is setting a status that the worker acts on.
- The perspective only renders notes, so it still works without the backend (offline, in the browser alone, on Android, in Demo).
- Gestalts should separate the keys the worker owns from the keys the user owns, or use separate notes, so the per-key 3-way merge rarely conflicts.
- Results become ordinary gestalts (`flashcard@1` instead of an Anki deck), so other perspectives can use them.

**Live calls need a `services` port.** Some things need an answer in the UI right away: re-chunking a cloze, OCR on a region, "run now", progress. A perspective declares optional, versioned services next to `data`, e.g. `services: { sketchbook: 'sketchbook-worker@1' }`. The host app resolves them, and the perspective falls back gracefully when one is missing. The connector resolves them (`GET /services`) and proxies them (`/services/{name}/…`), so CORS, the local-only rule and tokens stay in one place.

**A plugin is a bundle**, with a manifest, gestalts, an optional UI part (perspectives and panes) and an optional worker:

```yaml
# plugins/<id>/plugin.yml
id: sketchbook
version: 1.2.0
gestalts: [sketchbook-page@1, card-proposal@1, flashcard@1]
worker:
  runtime: uv                      # or: binary, container
  entry: worker/main.py
  triggers:
    - { on: change, query: { gestalt: sketchbook-page@1, where: { status: queued } } }
    - { on: schedule, cron: "0 6 * * *" }
  services: [sketchbook-worker@1]
  access:
    read:  { gestalts: [sketchbook-page@1, flashcard@1] }
    write: { tagsAll: [sketchbook] }
    network: [api.anthropic.com]
```

A worker has the same shape as a perspective: declared queries go in, notes come out. It can run on a change, on a schedule, or as a long-running service.

**The desktop connector hosts plugins.** It installs workers (`uv sync` from their lockfile), watches the folder for trigger queries, runs workers and proxies services. Each worker gets a token limited to its `access`, so plugin workers go through the connector API instead of writing the folder directly. A worker may also run on another machine (e.g. a GPU box) and register with the connector by URL. Where no host exists, the UI part still works on whatever notes have synced.

Plugins could live in the data folder (`plugins/`), so they sync across devices. Their workers only run where there is a host.

**Trust.** A UI part runs in the browser and can be sandboxed (an iframe, with `data`/`repo` passed over `postMessage`). A worker is native code with full access to the machine: `access` limits what it can do to notes, nothing else. Plugins you wrote or vibe-coded yourself are trusted when they're dropped into `plugins/`. Third-party workers need an explicit approval step or a container runtime.

**Open questions:**
- Change notifications: the connector has no watch/SSE endpoint, and 15 s sync is slow for progress updates.
- Volume: `Repo` loads every note, so high-volume workers need retention or digest notes.
- Python validating gestalts: `mise run lint-data` for now, maybe a pyo3 binding of the core later.
- Where secrets live: with the worker, never in notes.
- Publish the Python plugin SDK to PyPI. Built-in plugins depend on it by repo path, so the connector embeds `sdk/python` next to them; third-party plugins need a released version.
- CI release builds of the connector on macOS and Windows runners (their tray/event-loop code is untested so far), plus code signing.
- Safari blocks `https://` pages from calling `http://127.0.0.1`; supporting it needs TLS on localhost.
