## [Unreleased]

- Lintus is now a single binary, rewritten in Rust: no Ruby needed. Install it with the
  script or the archives on the GitHub releases, or `cargo install lintus`. Config files,
  flags, output formats and exit statuses are unchanged, checked by a conformance suite that
  both implementations pass.
- `lintus auth login`, `status` and `logout` save the Jev API key in the user's config
  directory. A run takes the key from `--api-key`, then `JEV_API_KEY`, then the saved key.
- `JEV_API_URL` sends requests to another endpoint, such as a proxy.
- The GitHub Action downloads the binary instead of setting up Ruby; its `ruby-version`
  input is ignored. The pre-commit hook builds lintus from source.
- A request that cannot reach the API fails its file instead of stopping the run.
- `--diff` and `--staged` together are an error; before, the last one given won.
- Skipped and failed files are listed in path order, not in the order they finished.
- The Jev client ships as its own crate, `jev-api`.
- The Ruby implementation is removed. The `lintus` gem stays at 0.2.0, and is no longer
  updated.

## [0.2.0] - 2026-09-21

- A progress counter on stderr while files are being checked.
- The JSON output lists every file with the probability each rule gave it, not only offenses.
- The starter config's documentation rule asks about the offense and scopes it with criteria,
  so namespace wrappers no longer trip it.
- Removed rule `severity` and the `--fail-on` flag: an offense is an offense. A config that still
  sets `severity` is rejected with a message naming the key.

## [0.1.0] - 2026-09-21

- Initial release.
- Rules are read from `.lintus.yml` at the repository root and asked to the Jev model as noul questions.
- Per-rule `paths`, `exclude`, `criteria`, `threshold`, `severity` and `offense_when`.
- File selection: whole tree, explicit files or directories, `--diff [REF]`, and `--staged`.
- Output formats: `text`, `github` (workflow annotations) and `json`.
- `lintus init` writes a starter config; `--list` shows what would be sent without calling the API.
- A composite GitHub Action (`action.yml`) and a pre-commit hook definition.
