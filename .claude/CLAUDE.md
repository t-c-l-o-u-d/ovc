# CLAUDE.md

## Linting

Do not run linters (shellcheck, shellharden, hadolint,
markdownlint, yamllint, etc.) directly. The pre-commit
hook runs `bash linter-aio.bash fix` then
`bash linter-aio.bash lint` automatically on every
commit.

The script auto-detects the worktree via `git rev-parse`.
It does not accept a directory argument.

## Shell commands

Run all commands directly from the repo root
(e.g. `git status`, `python3 scripts/rebuild.py`).
Never use `git -C <path>` or prefix commands with
absolute paths — the working directory is already
the repo root.

- Never truncate command output. Always capture
  full output for debugging.

## Research

- Always check `--help` and `man` pages locally
  before consulting web documentation.
- Document findings in a markdown file to easily
  resume troubleshooting if interrupted.

## Git

- Create GitHub issues for planned work. Work on a
  feature branch and open a pull request that
  references the issue to close it.
- Commit messages must be 12 words or fewer.
- Never include a `Co-Authored-By` line or give
  Claude credit.
- Always make small, focused commits — one logical
  change per commit.
- Commit frequently. Do not batch multiple changes —
  commit each logical change as soon as it's done
  and passing lint.
- Push commits after they pass the pre-commit hook.
- When renaming or moving files/directories, include
  both the old and new paths in the **same commit**
  so git detects the rename.
- Never drop or clear git stashes without explicit
  user approval. Always ask first.

## Releases

Releases are cut via the GitHub Actions `release.yaml`
workflow dispatch. Never bump `Cargo.toml` version
locally — the workflow handles it.

To release, run:

```sh
gh workflow run release.yaml -f version=X.Y.Z
```

- **Patch (Z)**: bug fixes, output tweaks, CI/metadata
  changes, dependency bumps.
- **Minor (Y)**: new user-facing features or flags.
- **Major (X)**: breaking changes to CLI behavior or
  output that existing scripts depend on.

After triggering, update the release notes on GitHub
with a human-readable summary grouped by category
(e.g. Features, Fixes, Internal).

## Documentation

- Keep all markdown files (`README.md`, `docs/`)
  in sync with the codebase.
- When a change affects documented behavior, update
  the relevant docs in the same commit.
- Outdated docs are treated as bugs.
- Every plan and every commit must account for doc
  updates. Check `README.md` and `docs/` for any
  text that the change invalidates or that should
  reference the new work.
- Only document code that is hard to understand.

## Code quality

Treat this as production-grade software. Strict code
quality and maintainability are non-negotiable. All
linters must pass; no warnings, no exceptions. Write
clean well-structured code on every change.

- Follow the Unix Philosophy: do one thing and do
  it well. Apply this to functions.
- Prefer the standard library over third-party
  dependencies.
- Every code change must have corresponding tests.
  If a test is missing, write one before committing.
- Each commit must result in a fully functional
  program that passes all checks.
- Never suppress, disable, or relax linter rules
  without explicit user approval. This includes
  `shellcheck disable`, yamllint config overrides,
  ignore directives, and any mechanism that weakens
  a check. Always present the finding and proposed
  override, then wait for approval before applying.
