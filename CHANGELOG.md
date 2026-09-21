## [Unreleased]

- A progress counter on stderr while files are being checked.
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
