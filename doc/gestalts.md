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
- Collections: `string_list`, and `map`, which takes nested `fields`.
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
| `fsrs-state@1` | tag `fsrs-state`, one relationship `reviews`, `model` (e.g. `FSRS-7`), `last_rating` 1–4, `card` (a ts-fsrs v6 card) |

Learner state is a separate note that `reviews` the card. It is not stored in the card itself. That way content can stay read-only (shipped in Demo's `public/`) while each learner's progress lives elsewhere.
