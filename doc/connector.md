# Desktop connector and sync

```sh
just connector ~/my-gestalt-data          # or: gestalts-connector --data DIR [--port 7717] [--allow-origin URL]...
just lint-data ~/my-gestalt-data          # gestalts and lints per note
```

The connector listens on `127.0.0.1` only. CORS is limited to `http(s)://localhost:*`, `127.0.0.1:*`, and any `--allow-origin`. There is no authentication: anything that runs on this machine with an allowed origin can read and write the folder.

| endpoint | |
|---|---|
| `GET /health` | `{app, version, root}` |
| `GET /notes` | `[{key, text}]` — all notes |
| `GET/PUT/DELETE /notes/{key}` | raw YAML text |
| `GET /blobs` | names |
| `GET/PUT /blobs/{name}` | bytes |

## Sync

The apps sync their writable OPFS notes with the folder at these times:
- on connect
- 0.8 s after each write
- every 15 s

Each side is compared with a **base**: the state after the last sync, stored in OPFS per connector URL. Each note is then merged:

- Only one side changed since the base: take that side, including deletes.
- Both sides changed, and both are YAML maps: merge per top-level key. A key changed on both sides is a conflict, and the local version wins.
- Both sides changed in some other way (unparseable, not maps, or created on both sides): the local version wins, and it counts as a conflict.
- One side deleted and the other edited: the edit wins.

On a conflict the remote text is saved as the note `<key>.conflict-<timestamp>` on both sides, and the connector settings list it. Blobs are only ever copied to the side that lacks them. They are never merged or deleted.

Caveats:
- The Demo pulls every note and blob in the folder into its OPFS layer. Of the shipped words, only those the learner rated (and their blobs) are in that layer, so only those reach the folder.
- A Demo hosted on a public origin that calls `localhost` runs into Chrome's Private Network Access rules. The connector does answer the preflight, but the prototype is only tested on local origins.
