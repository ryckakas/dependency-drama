# Changelog

[← Back to README](README.md)

Notable changes per release, in the format of [Keep a Changelog](https://keepachangelog.com).
This project follows [Semantic Versioning](https://semver.org). While it is pre-1.0, a
backwards-compatible change is a patch bump (0.1.0 → 0.1.1), and a change that breaks a flag, the
JSON output or the GitHub Action's inputs moves to the next minor (0.1.x → 0.2.0). A changed or
added joke is never a breaking change.

`dist` reads the section matching a tag and uses it as that release's notes, so an entry here is
what a user reads on the GitHub release page.

## [0.1.1] - 2026-11-13

Fixes from a trial on a pnpm monorepo of 33 workspaces and about 3,000 packages.

### Added

- **Every change in a pull request is named.** Changes that get no scene of their own are listed
  in one closing line, such as "Also tonight: @types/dompurify 3.2.0 written out." JSON lists
  them as `untold`.
- **Partial upgrades are told as partial.** When only some workspaces move a dependency up a
  major version, the scene says so: "Only 1 of the 16 workspaces that use it made the move."
- **A rerun folds away.** When a pull request leaves "Previously on" exactly as it was, the
  Markdown comment folds that part under "The same as before this change", so Tonight comes
  first. JSON marks it with `rerun: true`.

### Changed

- **Every general line is rewritten**, inside one frame: a TV show. Each line ends on a turn, and
  a deprecated package's own notice sits in the middle of its line instead of ending it.
- **Lines scale with the fact.** A dependency 18 major versions behind, a package installed in
  many versions, one unchanged for eight years or a huge entourage gets lines written for that
  size, not the ones for a small gap.
- **Variety first.** Every kind of fact gets one scene before any kind gets a second, so a large
  project no longer gets the same five kinds of scene every time.
- **A pnpm project is named after its `package.json`**, not the directory it was cloned into, so
  CI and a local run tell the same episode.

### Fixed

- **Deprecation notices are no longer cut mid-word.** A long notice ends at a whole sentence, or
  else between two words.
- **Workspaces are compared one by one.** A major bump in one workspace was reported as if the
  whole project had moved, or not at all, depending on how the versions sorted.
- **The Action compares a head checkout with the merge base.** A workflow that checks out the pull
  request's head commit compared it with the base branch's tip, so changes merged there since
  the branch was cut showed up as undone by the pull request. The default checkout was already
  right.

## [0.1.0] - 2026-11-13

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
- **pnpm.** `pnpm-lock.yaml` from pnpm 9 or newer is read like npm's lockfile, workspaces
  included. It records neither funding requests nor install scripts, so those two scenes do not
  appear. Because it does not say which registry a package came from, the registry settings pnpm
  would use are read, and a name from a scope or default registry that is not `registry.npmjs.org`
  is never looked up.
- **npm workspaces.** The dependencies of every workspace member count as direct, not only the
  root's.
- **`--markdown` and `--json`.** Markdown for a pull request comment, with third-party text
  escaped; JSON with each scene's kind, the facts behind it and its line.
- **GitHub Action.** Writes the episode to the job summary and, on a pull request that changes
  the lockfile, posts it as one comment that is updated in place.
- **Lines for well-known packages.** About a dozen packages most developers know by name get
  lines of their own.
