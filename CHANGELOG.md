## [Unreleased]

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
