# Changelog

[← Back to README](README.md)

Notable changes per release, in the format of [Keep a Changelog](https://keepachangelog.com).
This project follows [Semantic Versioning](https://semver.org). While it is pre-1.0, a
backwards-compatible change is a patch bump (0.1.0 → 0.1.1), and a change that breaks a flag, the
JSON output or the GitHub Action's inputs moves to the next minor (0.1.x → 0.2.0). A changed or
added joke is never a breaking change.

`dist` reads the section matching a tag and uses it as that release's notes, so an entry here is
what a user reads on the GitHub release page.

## [Unreleased]

### Added

- **Episodes from an npm lockfile.** `dependency-drama [path]` reads `package-lock.json`
  (lockfileVersion 2 and 3) and tells up to five scenes, each a true fact: an entourage of
  transitive packages, direct dependencies still at 0.x, deprecated packages, several versions of
  one package, unsatisfied peer ranges, funding requests and install scripts. The same lockfile
  always gives the same episode.
- **Registry scenes.** Direct dependencies installed from `registry.npmjs.org` are looked up
  there to find the ones that are major versions behind or unchanged for three years. Names from
  a private registry, a git URL or a local path are never sent, and `--offline` sends nothing.
- **`--base <path>`.** Compared with the same project before a change, up to three more scenes
  tell which direct dependencies were added, removed or moved up a major version.
- **npm workspaces.** The dependencies of every workspace member count as direct, not only the
  root's.
- **`--markdown` and `--json`.** Markdown for a pull request comment, with third-party text
  escaped; JSON with each scene's kind, the facts behind it and its line.
- **GitHub Action.** Writes the episode to the job summary and, on a pull request that changes
  the lockfile, posts it as one comment that is updated in place.
- **Lines for well-known packages.** About a dozen packages most developers know by name get
  lines of their own.
