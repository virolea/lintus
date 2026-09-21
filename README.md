# Lintus

A linter whose rules are written in plain language.

Lintus reads a YAML file of rules at the root of your repository. Each rule is a question
about a file. For every file a rule applies to, Lintus asks the question to the
[Jev](https://github.com/virolea/jev) model as a *noul* (a true/false judgement with a
probability), and reports an offense wherever the answer says so.

```yaml
# .lintus.yml
rules:
  no_business_logic_in_controllers:
    description: Controllers should not implement business logic
    question: "Does this controller implement logic that sits outside from the following responsibilities: networking, authorization, delegation to domain models and response?"
    paths:
      - "app/controllers/**/*.rb"
```

```
$ lintus
app/jobs/retry_job.rb: [no_sleep_in_jobs] Background jobs must never block on sleep. (noul 0.94)

42 files inspected, 1 offense detected
```

It runs on the whole tree, on the files changed since a git ref, or on the files staged for a
commit, so it fits a CI job as well as a pre-commit hook.

## Installation

```bash
gem install lintus
```

Or add it to your `Gemfile`:

```ruby
gem "lintus", group: :development
```

Lintus needs a Jev API key. Export it as `JEV_API_KEY` or pass `--api-key`.

## Getting started

```bash
lintus init      # writes a starter .lintus.yml
lintus --list    # shows which files each rule would be asked about, without calling the API
lintus           # lints every file
```

## The config file

Lintus looks for `.lintus.yml` (or `lintus.yml`, `.lintus.yaml`, `lintus.yaml`) in the current
directory and its parents, the way git finds `.git`. The directory holding the file is the root
every path is relative to.

```yaml
# Globs every rule applies to, unless the rule has its own `paths`.
paths:
  - "app/**/*.rb"
  - "lib/**/*.rb"

# Globs no rule ever applies to.
exclude:
  - "vendor/**"
  - "db/schema.rb"

# Files larger than this many bytes are skipped instead of sent to the model. Default 100000.
max_file_size: 100000

rules:
  no_raw_sql:
    description: Build queries with Active Record, not string interpolation.
    question: Does this file build a SQL string by interpolating or concatenating values into it?
    criteria:
      "true": A SQL fragment is assembled from Ruby values with #{}, +, or format.
      "false": Queries go through Active Record methods or bound parameters, or there is no SQL.
    threshold: 0.7
    paths:
      - "app/**/*.rb"
    exclude:
      - "app/models/legacy/**"

  service_objects_are_documented:
    description: Service objects carry a comment explaining what they do.
    question: Does every class in this file have a comment describing its responsibility?
    offense_when: false
    paths:
      - "app/services/**/*.rb"
```

### Rule attributes

| Key            | Required | Meaning |
| -------------- | -------- | ------- |
| `question`     | yes      | The noul statement asked of the model. Phrase it so that `true` means the file is an offense. |
| `description`  | no       | The message printed for an offense. Defaults to the question. |
| `criteria`     | no       | A map with `"true"` and `"false"` keys spelling out what each answer means. This is how you scope an ambiguous question, and it is worth writing for every rule that matters. |
| `threshold`    | no       | Probability above which the answer counts as `true`. Defaults to 0.5. Raise it for rules where a false positive is costly. |
| `paths`        | no       | Globs the rule applies to. Defaults to the top-level `paths`; with neither, every file. |
| `exclude`      | no       | Globs the rule never applies to, on top of the top-level `exclude`. |
| `offense_when` | no       | `true` (default) or `false`. Set to `false` for rules phrased positively, such as "Does every class have a comment?". |

Rule ids are snake_case. They become the question identifiers in the Jev request and the
`[tag]` in the output.

### Globs

Globs use Ruby's `File.fnmatch` with pathname semantics: `*` does not cross directories,
`**/` does, and `{a,b}` alternation works. A bare directory (`vendor`) and a trailing `**`
(`vendor/**`) both match everything beneath it.

### Writing good rules

Every rule that applies to a file is sent in a single request, evaluated in parallel by the
model. So prefer several narrow questions over one broad one: "Does this file call `sleep`?"
and "Does this file rescue `Exception`?" as two rules beat "Does this file do anything a job
should not?".

**Ask about the offense, not about compliance.** "Does every class have a comment?" turns
false on a single edge case, and there is always one: the namespace wrapper, the reopened
class, the one-line error subclass. "Is there a class without a comment?" is the same
question, but its `criteria` can now spell out which classes count. Keep `offense_when: false`
for questions that are genuinely easier to phrase positively.

**Name the unit and list what to ignore.** The model reads a question literally. When Lintus
first linted itself with "does every class or module have a comment?", every file was flagged
with a probability around 0.15: each one opens with an undocumented `module Lintus`. The fix
was not the threshold but the criteria, which now exclude wrappers whose body only nests other
definitions.

**Read the probabilities before touching the threshold.** Scores clustered far from 0.5 mean
the model is sure of its reading of the question. If that reading is not yours, reword. Move
the threshold only when the scores of clean and offending files overlap around it.

Lintus sends the model the file's path and its full content. A question can therefore refer to
the file name ("Is this a controller?") as well as the code.

## Choosing files

| Command                     | Files |
| --------------------------- | ----- |
| `lintus`                    | Every file in the tree (tracked and untracked, honouring `.gitignore`). |
| `lintus app/jobs lib/x.rb`  | The given files and directories. |
| `lintus --diff`             | Files with uncommitted changes, plus untracked files. |
| `lintus --diff main`        | Files changed since the merge base with `main`, plus uncommitted and untracked changes. |
| `lintus --staged`           | Files staged for commit. The staged content is what gets linted, not the working tree. |

Only files that at least one rule applies to are sent. Deleted files are never sent.

## Output

`--format text` is the default. `--format github` prints GitHub Actions workflow commands, so
each offense becomes an annotation on the file in the pull request; it is the default when
`GITHUB_ACTIONS` is set. `--format json` is for other tools, and it also lists the probability
every rule gave every file under `files`, offense or not, which is what you want when tuning
a rule's wording or threshold.

Exit status is `0` when clean, `1` when there are offenses, and `2` when a request failed or
the invocation was wrong.

While a run is in flight, a counter on stderr shows how many files are done. Off a terminal
(CI logs, pipes) it is a single line announcing the run, so captured output stays clean.

## GitHub Actions

```yaml
# .github/workflows/lintus.yml
name: Lintus
on: [pull_request]

permissions:
  contents: read

jobs:
  lintus:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0   # needed to diff against the base branch
      - uses: virolea/lintus@v0.2.0
        with:
          jev-api-key: ${{ secrets.JEV_API_KEY }}
```

On pull requests the action lints only the files changed against the base branch, and
annotates them. Set `base: none` to lint everything, or `base: origin/develop` to diff
against another ref. Extra flags go in `args`. If the workflow already sets up Ruby, the
action reuses it.

## Pre-commit hook

With [pre-commit](https://pre-commit.com):

```yaml
repos:
  - repo: https://github.com/virolea/lintus
    rev: v0.2.0
    hooks:
      - id: lintus
```

pre-commit passes the staged file names, so only those are checked.

With a plain git hook, use `--staged` so the linted content is what is actually being
committed:

```bash
#!/bin/sh
# .git/hooks/pre-commit
exec lintus --staged
```

Either way `JEV_API_KEY` must be in the environment of the shell running the commit.

## Other options

```
-c, --config PATH      Config file to use instead of searching for one
-j, --jobs N           Concurrent requests to the Jev API (default 4)
    --api-key KEY      Jev API key (default: $JEV_API_KEY)
-l, --list             Show what would be checked without calling the API
```

Rate-limited and overloaded responses are retried with exponential backoff, three times per
file, before the file is reported as failed.

## Development

After checking out the repo, run `bin/setup` to install dependencies, then `bundle exec rake`
to run the tests and RuboCop. `bin/console` gives you an IRB session with the gem loaded.

The repository lints itself: see `.lintus.yml` and `.github/workflows/lintus.yml`.

## Contributing

Bug reports and pull requests are welcome on GitHub at https://github.com/virolea/lintus.

## License

The gem is available as open source under the terms of the [MIT License](https://opensource.org/licenses/MIT).
