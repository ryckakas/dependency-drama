# dependency-drama

Your dependencies, narrated as a soap opera. Every line is a true fact from the lockfile.

```
PREVIOUSLY ON shop-web…

  left-pad 1.3.0 has been deprecated, and nobody has asked it to leave. The note on its door: "use String.prototype.padStart()"
  request is deprecated and still installed. It left a note: "request has been deprecated, see https://github.com/request/request/issues/3142"
  express 4.22.3 still thinks it is the latest. The family has not mentioned 5.3.0.
  webpack is 1 major version behind. Nobody has told it about 5.112.2.
  You invited 9 guests. 461 showed up.
```

It is a toy. It is also, by accident, a readable summary of what is rotting in your `node_modules`.

## Status

Early. It reads npm's `package-lock.json` (npm 7 or newer) and prints an episode. There are no
released binaries yet, so it is built from source with a current Rust toolchain:

```bash
cargo install --git https://github.com/ryckakas/dependency-drama
```

## Usage

```bash
dependency-drama              # the project in the current directory
dependency-drama path/to/app  # another project, or a package-lock.json
dependency-drama --markdown   # Markdown, for a pull request comment
dependency-drama --offline    # ask the npm registry nothing
```

## What it looks for

| Scene | The fact behind it |
|---|---|
| Entourage | Installed packages far outnumber the ones you asked for |
| Almost ready | A direct dependency still at version 0.x |
| Ghost | A package that is deprecated and still installed |
| Clones | Several versions of the same package |
| Feud | A peer dependency range the installed version does not satisfy |
| Time capsule | A direct dependency one or more major versions behind |
| Missing person | A direct dependency whose registry entry has not changed in three years |
| Collection plate | Packages asking for funding (`npm fund`) |
| Night visitor | Packages that run a script on install |

An episode has five scenes at most. The same lockfile always gives the same episode.

## What leaves your machine

To learn how far behind a dependency is, the tool asks `registry.npmjs.org` about each **direct**
dependency that was installed from `registry.npmjs.org`: one request per package, containing the
package name. Packages from a private registry, a git URL or a local path are never looked up, and
nothing else is sent. `--offline` sends nothing at all, and the two registry scenes are left out.

## Writing lines

The jokes live in plain text files under [`src/lines`](src/lines), one line per row, with
`{placeholders}` for the facts. No Rust is needed to add one. Two rules:

- The joke is on the package or on the project, never on a person. Maintainers owe nobody anything.
- Keep it deadpan and safe to paste into a work chat.

A test fails if a line uses a placeholder its scene does not provide.

## License

MIT
