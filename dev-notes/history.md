# History

Developer notes moved out of CLAUDE.md: why some things are where they are.

## Moved out of the browser

The service layer was extracted so that no rule lives in JavaScript; a native egui front end was built on it and then dropped as not worth maintaining. What stayed:

- **Formatting for anything rendered in Rust lives in the library's `format` module** (dates in local time via `SystemTimeToTzSpecificLocalTime` — the DST rule of *that* date — lap times, credits, track names), matching `utils.js`.
- **The HTML export is built by `service::export`** (`App::career_export_html`). The web page's "Download HTML" just fetches `GET /api/career/export`. It renders each race's lap chart from its file — the old browser-built export opened every `<details>` but the charts were fetched lazily, so they came out empty.
- The session picker's list is built server-side (`App::available_sessions`, `GET /api/championships/:id/available-sessions`), from the same verdict adding a session applies.
- **Two web-page bugs fixed alongside:** "Tear up" could never appear (`data.open || !data.offers` — `open` is false for every signed season, and `[]` is truthy), so it is now offered until the season's first session, the rule `unsign` enforces; and "Every recorded session raced the full roster" no longer shows for a season with no sessions.
- **Testing roster edits by hand:** run a copy of the server against a scratch install — `UserData/CustomAIDrivers` as a *copy*, `GUI` and `Vehicles` as directory junctions to the real install (class registry, livery manifests, pictures) — and remove the junctions with `rmdir`, never a recursive delete. Don't edit the copies with Git Bash `sed -i`: it rewrites CRLF to LF across the whole file. The writer itself preserves CRLF (a per-track removal came back byte-identical).


