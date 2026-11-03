# Desktop connector and sync

The connector is a desktop app that makes a folder on this computer the source of truth for the web apps, and runs
their plugins. Install it from a package (`mise run package` / `mise run install`, see the README), launch it from the
app menu, and choose your folder in its window. It remembers the folder and lives in the tray from then on.

```sh
gestalts-connector                    # the desktop app: tray + status window, one instance per user
gestalts-connector --background       # same, without opening the window ("Start on login" uses this)
gestalts-connector serve [--data DIR] # headless; the folder from --data, else the remembered one
gestalts-connector lint DIR           # gestalts and lints per note
gestalts-connector publish DIR        # prepare a folder as a read-only deck
# all of them: [--port 7717] [--profile NAME] [--plugins-dir DIR]... [--allow-origin URL]...
```

In development, `mise run dev` starts it with `--profile dev --plugins-dir plugins`. The dev profile keeps its settings
apart from an installed connector. Both use port 7717, so quit one before starting the other.

## The desktop app

- **Tray:** shows the folder and has "Show status…", "Choose folder…" and "Quit". Clicking the icon opens the status
  window. On Linux the tray is a StatusNotifierItem over D-Bus, so there's no GTK dependency. Desktops without a tray
  (e.g. GNOME without an extension) still get the window.
- **Status window:** a separate `gestalts-connector status` process, so closing it never affects the server. It shows
  the folder (choose another one, open it), the web apps connected in the last minute, uv and each plugin's state,
  "Start on login", and "Quit connector".
- **One instance:** launching it again while it runs makes the running one show its window, then exits.

## Folders

A data folder holds `notes/`, `blobs/` and `.gestalts/folder-id`, a UUID created when the folder is first opened.
`/health` reports `folder: {id, path}`, or `null` while none is chosen; until then `/notes*` and `/blobs*` answer
`409`. The web apps keep their sync base per connector URL *and* folder id. Switching folders behind the same URL
therefore can't make one folder's missing notes look like deletions in another.

## Where it keeps its files

All per user, through the OS's standard locations (XDG on Linux, Application Support on macOS, AppData on Windows),
under `gestalts` (or `gestalts-<profile>`):

| | |
|---|---|
| config | `config.json` (`folder`, `extra_origins`), `control-token` |
| data | `builtin-plugins/<hash>/` (unpacked from the binary), `plugins/` (your own), `uv/`, `python/` |
| cache | `plugins/<id>/venv`, uv's download cache |

## Plugins

Built-in plugins (the repo's `plugins/`, plus the Python SDK they use) are compiled into the binary and unpacked on
start. Plugins you add go in the data folder's `plugins/`. `--plugins-dir` replaces both, which is useful when
developing plugins.

Workers are Python projects run in locked uv environments: `uv sync --locked`, then the environment's own executable,
which gets scoped, process-lifetime credentials. The connector uses `GESTALTS_UV`, else a uv ≥ 0.9 on `PATH`, else it
downloads a pinned uv release (checksum-verified) into its data folder. uv in turn fetches Python if none is installed,
so users install nothing.

`GET /plugins` reports worker health, `POST /plugins/{id}/runs` starts a manual run, `GET /services` discovers
services, and `/services/{id}/…` proxies them (with the query string and `Content-Type`; a service gets 30 s before the
app sees a 504). A worker whose process has ended is restarted on the next run or service request, and its error
says so meanwhile.

Built-in plugins:
- `conformance`: exercises the plugin contract.
- `typst`: the `typst@1` service. `POST /services/typst@1/compile` with `{source, assets?: {path: blob}, bib?, format:
  "svg" | "pdf"}` compiles in a fresh folder (assets at their paths, `bib` as `/sources.bib`, system fonts ignored)
  and answers SVG pages as JSON, the PDF, or 422 `{diagnostics, missing}`. It reads `typst-document@1` notes and the
  blobs they reference, and writes nothing.

## Access

The connector listens on `127.0.0.1` only. CORS allows `http(s)://localhost:*` and `127.0.0.1:*`, the official
frontends (`https://gestalter.koljasam.com`), `extra_origins` from the config, and any `--allow-origin`. The notes API
has no authentication: anything on this machine with an allowed origin can read and write the folder.

`/control/*` (status, choose folder, show window, quit) is only for the connector's own window. It needs the per-run
token from `control-token` in the `x-gestalts-control` header, and rejects any request that carries an `Origin`, which
is every request a web page can make.

| endpoint | |
|---|---|
| `GET /health` | `{app, version, folder: {id, path} \| null}` |
| `GET /notes` | `[{key, text}]` — all notes |
| `GET/PUT/DELETE /notes/{key}` | raw YAML text |
| `GET /blobs` | names |
| `GET/PUT /blobs/{name}` | bytes |
| `GET /control/status`, `POST /control/folder {path}`, `POST /control/show`, `POST /control/quit` | the connector's own window only (see above) |

## Sync

The apps sync their writable OPFS notes with the folder at these times:
- on connect
- 0.8 s after each write
- every 15 s

Each side is compared with a **base**: the state after the last sync, stored in OPFS per connector URL and folder id. Each note is then merged:

- Only one side changed since the base: take that side, including deletes.
- Both sides changed, and both are YAML maps: merge per top-level key. A key changed on both sides is a conflict, and the local version wins.
- Both sides changed in some other way (unparseable, not maps, or created on both sides): the local version wins, and it counts as a conflict.
- One side deleted and the other edited: the edit wins.

On a conflict the remote text is saved as the note `<key>.conflict-<timestamp>` on both sides, and the connector settings list it. Blobs are only ever copied to the side that lacks them. They are never merged or deleted.

Caveats:
- The Demo pulls every note and blob in the folder into its OPFS layer. Of the shipped words, only those the learner rated (and their blobs) are in that layer, so only those reach the folder.
- A Demo hosted on a public origin that calls `localhost` runs into Chrome's Private Network Access rules. The connector does answer the preflight, but the prototype is only tested on local origins.
