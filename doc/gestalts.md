# Gestalts

A gestalt is a named, versioned shape a note may have.
Each version lives in its own file, `gestalts/<name>/v<N>.yml`, and the core embeds them at build time. Add new ones to `BUILTIN` in `crates/core/src/gestalt.rs`.

A note is a **candidate** for a gestalt when:
- it has every tag in `requires_tags`, and
- it has at least one of the `trigger_fields`, if any are listed.

A candidate **has** the gestalt when every rule holds. A candidate that breaks a rule gets lints instead, which the editor shows. A note can have several gestalts at once.

```yaml
name: pronunciation-card
version: 1
requires_tags: [pronunciation-card]
trigger_fields: []          # e.g. [url] for gestalts without a tag
fields:
  title: { type: string, required: true }
blobs:                      # counts by label; `_` = unlabelled
  audio: { min: 1, max: 1 }
relationships:
  reviews: { min: 1, max: 1 }
```

Field types:
- Strings: `string` (one line) and `text`.
- Numbers: `number` and `integer`.
- `bool`, `date` (`YYYY-MM-DD`), `datetime` (ISO 8601), `url`.
- `partial_date`: `YYYY`, `YYYY-MM` or `YYYY-MM-DD`, or a range of two joined by `/` (either side may be open).
- Collections: `string_list`; `list`, whose `items` rule every element follows; and `map`, which takes nested `fields`.
- `any`.

Field options:
- `required`.
- `min` and `max`: the value for numbers, the item count for lists.
- `one_of`.

## Versioning

- Consumers always ask for `name@N`.
- A breaking change gets a new `vN+1.yml`. The old file stays, so existing perspectives keep working, and a note may have both versions.
- Never change the meaning of a published version.

## Defined

| gestalt | |
|---|---|
| `flashcard@1` | tag `flashcard`, `front` and `back` |
| `appointment@1` | tag `appointment`, `date` |
| `link@1` | a well-formed `url` |
| `pronunciation-card@1` | tag `pronunciation-card`, `title` (the term), blob `audio`, optional blob `image`, optional `lang` |
| `fsrs-state@1` | tag `fsrs-state`, one relationship `reviews`, `model` (e.g. `FSRS-7`), `last_rating` 1–4, `card` (a ts-fsrs v6 card), optional `facet` |
| `recurring@1` | tag `recurring`, `weekday` (`mo`…`su`), optional `until` (partial date) |
| `daily@1` | tag `daily`, `date`; keyed `stableId("daily", date)`, so one note per date across devices |
| `project@1` / `issue@1` | tag `project` / tag `issue` with at least one `project` relationship |
| `published@1` | `publish_route` and `publish`: a note's text as published, apart from `main` |
| `template@1` | tag `template`, `format` (`jinja-html`), `source`; superseded by `template@2` |
| `source@1` | tag `source`, `title`, `citekey`, `entrytype` and biblatex fields (see [Sources](#sources)) |
| `canvas@1` | tag `canvas`, `nodes` (each `id`, `x`, `y` and `text` or `note`), one `canvas-of` relationship; superseded by `canvas@2` |
| `canvas@2` | a `canvas` key with `nodes` (as in v1) on the note it's about; its linked notes are `on-canvas` relationships of that note |
| `template@2` | a `template` key `{format, source, name}` on the note it renders |
| `cards@1` | a `cards` key: a list of `{id, front, back}`; `id` (dsl's `card_id` of the front when made) is the `facet` of the card's `fsrs-state` |
| `typst-document@1` | tag `typst`, `source`, `assets` (path → blob), blob `pdf` (at most one), `document-of` at most one |
| `term@1` | tag `term`, `title` (the term), optional `lang`, `translations`, `transliteration`, `examples` (each `text`, `translation`, `credit`), `value` (a numeral's number); blobs `image`, `audio`, `doodle` at most one each; alternate spellings in `aliases` |
| `sentence@1` | tag `sentence`, `title` (the sentence), optional `lang`, `translations`, `transcription`, `credits` (each `for`, `text`), `parts` (each `content`, `translations`, `transcription`, `examples`, `term` = a `term@1` id), `gaps` (each `word`, `at`, `distractors`), `start`/`end` in seconds with at most one `segment-of` |
| `quote@1` | tag `quote`, `main`, optional `attribution`; may `cites` a `source@1` |
| `country@1` | tag `country`, `title`, `code` (ISO alpha-3), optional `alpha2`, `priority`, `world_map` (map), blob `flag` at most one; `distractor` relationships to other countries |
| `scene-object@1` | tag `scene-object`, `title`, optional `props`, `hold` (maps), blob `model` (glb) or `image` |
| `interaction@1` | tag `interaction`, `action`, optional `effects`, `instructions` (lang → sentences); one `actor`, at most one `target` |
| `practice-state@1` | tag `practice-state`, one `reviews`, `model` (a scheduler other than FSRS: `ebisu`, `streak`, …), `state` (map), optional `facet` |
| `practice-session@1` | tag `practice-session`, `app` (perspective id), `started`, optional `ended` (datetimes), `active_ms`, `trials`, `events` (each with `at`); random key, one writer |
| `settings@1` | tag `settings`, `scope` (perspective id), further fields as the scope defines; keyed `stableId("settings", scope)` |

Relationship labels with a meaning: `inline` (a `[[reference]]` in the note's text, kept in step with it), `card-of` (a flashcard extracted from a note, the old form of `cards`), `canvas-of`
(the old form of a note's canvas) and `on-canvas` (each note on a canvas), `document-of` and `cites` (a Typst document or quote, its note and the sources it cites),
`template-of`, `reviews` (learner state and the note it's for), `segment-of` (a sentence and the recording it's spoken
in), `distractor` (a country easily confused with this one), `member` (a group and its notes), `actor` and `target`
(an interaction and its scene objects).

## Folded parts

A note's canvas, template and cards were attached notes (`canvas@1` `canvas-of`, `template@1` `template-of`,
`flashcard@1` `card-of` it). They're keys on the note now, so they merge per key with the rest of it. Migrated data
isn't rewritten. Readers take the note's key first, else the attached notes (`packages/client/src/fold.ts`). Every
save through the new form writes the key and retires the attached notes: tag `retired` plus
`folded: {into: <host id>, hash: <sha256 of the part as folded>, card: <id>}` (`card` for cards only). A retired
note whose part no longer matches `hash` was edited after the fold (another device, the YAML editor), and the
editors offer it back: "Use that version" or "Keep this one".

## Sources

`source@1` is the biblatex model: `entrytype` (`article`, `book`, `incollection`, `inproceedings`, `thesis`, `report`,
`online`, `software`, `misc`), names (`author`, `editor`) as `family, given`, `family, suffix, given` or one name
without a comma (an organisation), lists (`publisher`, `institution`, `organization`, `location`), partial dates
(`date`, `urldate`) and literals. Braces protect case as in BibTeX (`{NASA}`), `$…$` is math. `bib_extra` keeps
imported fields the model has no place for, so they're written back on export. The `citekey` should be unique;
the Sources pane flags duplicates.

`crates/core/src/bib.rs` reads BibTeX (legacy types and fields mapped: `journal` → `journaltitle`, `year`+`month` →
`date`, `phdthesis` → `thesis` + `type`), writes biblatex, finds the source an entry probably is (a shared DOI, ISBN or
eprint decides; else title, first author and year, loosely), merges an entry into a source, and lists the keys a Typst
document cites. The client exposes it as `bib`.

## Learner state

Learner state is a separate note that `reviews` the card. It is not stored in the card itself. That way content can stay read-only (shipped in Demo's `public/`) while each learner's progress lives elsewhere.

State notes (`fsrs-state@1`, `practice-state@1`) have stable keys, so two devices rating the same card write one note:
`stateKey(gestalt, target, facet?)` in the client (`state_key` in the core), which is `stableId(gestalt, target)` or,
for one facet of a note (a cloze level, a map exercise), `stableId(gestalt, "<target>/<facet>")`. States saved
before this keep their random keys. Logs go in `practice-session@1` notes instead, one per session.

Perspectives share this through `@gestalts/client` (`learning.ts`, `session.ts`, `practice.tsx`):
- `fsrsStates(notes)` indexes `fsrs-state@1` notes by `{target, facet}`. Facets of a note are separate, and a state
  without `facet` is the whole note's. If two notes hold the same one (a random key next to the stable one after a
  sync), the later review wins, then more reps, then the greater key, so every device agrees.
- `review(scheduler, prev, grade, now)` is plain FSRS. A perspective may adjust the result (Greek alphabet makes a card
  due right away when it starts or stops struggling). `saveReview(repo, facet, prev, next, log)` writes it under the
  existing key or `stateKey`, and logs the review.
- `usePracticeSession(app)` writes this page's `practice-session@1` notes. A session starts at its first trial, so
  opening a perspective writes nothing, and it includes the activity just before that trial. Any key or pointer press
  while the page is visible counts as activity. A gap between two activities counts toward `active_ms` up to 60 s,
  and time while hidden counts nothing. After 10 minutes without activity the next trial starts a new session.
  `ended` is the last activity, updated on every write, so a closed tab still leaves a finished session.
  `shown()` marks when a card appears; `trial(event)` appends `{at, ms, …event}`, `ms` being the time since `shown()`.
- A review event is `{at, ms, kind: fsrs-review, target, facet?, rating}`. It replaces the review log py-fsrs kept in
  its state file. Other apps add their own `kind`s.
