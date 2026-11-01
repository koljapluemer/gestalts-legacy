# Data format

```
my-gestalt-data/
  notes/<key>.yml
  blobs/<name>
  index.json        # only when published as a static site: {"notes": [keys], "blobs": [names]}
```

A note is any YAML. Conventions, all optional:

| field | meaning |
|---|---|
| `id` | global uuid, normally also the file name |
| `title` | one line |
| `main` | the content, usually multi-line |
| `aliases` | other names, list of strings |
| `tags` | contexts the note is (not) used in; letters, digits, dashes |
| `relationships` | list of `<uuid>` or `{to: <uuid>, label: <label>}` |
| `blobs` | list of `<file>` or `{file: <file>, label: <label>}` |

How the core copes with missing or broken pieces:
- A note without an `id` is addressed by its key, the file name without `.yml`.
- A file name that isn't a uuid is fine.
- Text that doesn't parse is kept as it is and reported as an error, so the universal editor can still fix it.
- An empty file counts as an empty map.

Keys and blob names must not contain `/`, `\` or control characters, and must not start with `.`.
