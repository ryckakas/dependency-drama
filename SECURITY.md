# Security policy

## Supported versions

Only the latest release gets security fixes. dependency-drama is pre-1.0, so a fix ships as a new
release rather than a backport; upgrade to pick it up.

## Reporting a vulnerability

Report it privately through GitHub:
**[Report a vulnerability](https://github.com/ryckakas/dependency-drama/security/advisories/new)**.
Please don't open a public issue, pull request or discussion for it.

A useful report says which version and install source you used (npm, Homebrew, Cargo, the
installer script or the GitHub Action), what you ran, and the input that triggers it. A minimal
`package-lock.json` or `pnpm-lock.yaml` that reproduces the problem is the most helpful thing you can attach.

The report stays private while it is investigated and fixed. Once a fixed release is out, the
advisory is published with credit to you, unless you'd rather not be named.

## What counts

dependency-drama promises three things about a lockfile it does not control, such as one in a
pull request from a fork. Anything that breaks one of them is in scope:

- **Reading is safe.** A lockfile is parsed as data and nothing in it is executed. A crafted
  lockfile that runs code, reads or writes outside what the run was pointed at, or crashes or
  hangs the process is a vulnerability.
- **The output is inert.** Package names, the project name and deprecation messages are written
  by strangers. In the terminal and Markdown output, and in the `text` and `markdown` fields of
  the JSON, they must not be able to inject markup, mention a user, or send terminal escape codes.
  The `facts` in the JSON are raw by design and documented as such.
- **Only public package names leave the machine.** The registry is asked about direct
  dependencies that were installed from `registry.npmjs.org`, and nothing else. A name from a
  private registry, a git URL or a local path being sent anywhere is a vulnerability, as is any
  request made with `--offline`.

A problem in a release artifact or the way it is built and published (the binaries, the npm
package, the Homebrew formula, the installer scripts or the GitHub Action) is also in scope.

A vulnerability in a dependency that dependency-drama doesn't actually reach is better raised as a
normal issue, or left to Dependabot, which already watches the dependencies.
