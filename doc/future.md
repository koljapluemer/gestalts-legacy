# Not built yet

- **Android connector**: the same `Storage` port over Android storage. The core has no desktop-only dependencies.
- **Obsidian two-way sync**: likely a converter between Markdown with frontmatter and notes, plus the existing `sync`.
- **Browser plugin** (e.g. vocabulary from Duolingo): it would write through the connector API.
- Modification times in the `Storage` port, which the search would need for a "recent" list.
- Authentication for the connector (a token per app).
- Learner activity and time statistics. The original sam-learns app had these; they weren't migrated.
