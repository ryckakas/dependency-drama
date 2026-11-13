# Contributing

The most useful contribution is a better line. It needs no Rust.

## Adding or changing a line

Lines live in [`src/lines`](src/lines), one file per scene type, one line per row. A row starting
with `#` is a comment. Facts go in with `{placeholders}`:

| File | Placeholders |
|---|---|
| `entourage.txt` | `{guests}` (such as "9 guests"), `{installed}` |
| `almost_ready.txt` | `{name}`, `{version}` |
| `ghost.txt` | `{name}`, `{version}`, `{message}` (the deprecation message) |
| `clones.txt` | `{name}`, `{versions}` (a count) |
| `feud.txt` | `{name}`, `{peer}`, `{range}`, `{installed}` |
| `time_capsule.txt` | `{name}`, `{version}`, `{latest}`, `{majors}` (such as "2 major versions") |
| `missing_person.txt` | `{name}`, `{year}`, `{years}` |
| `collection_plate.txt` | `{packages}` (such as "46 packages") |
| `night_visitor.txt` | `{name}`, `{packages}` |
| `new_arrival.txt`, `written_out.txt` | `{name}`, `{version}` |
| `recast.txt` | `{name}`, `{from}`, `{to}` |
| `quiet.txt` | none |

`cargo test` fails if a line uses a placeholder its scene does not provide, or if the same line is
written twice.

### What makes a line fit

- **It is true for every project the scene fires on.** The placeholders are the only facts the
  line has. "It has been here for years" is a guess; "It is at 0.4.2" is a fact. Counts can be 1:
  `{packages}` may read "1 package", so the sentence has to work for one and for many.
- **The joke is on the package or on the project, never on a person.** Maintainers owe nobody
  anything. A package that has not changed in years may simply be finished.
- **Deadpan, and safe to paste into a work chat.**
- **One or two sentences.** It is read in a pull request comment.

### Lines for well-known packages

[`celebrities.txt`](src/lines/celebrities.txt) has rows of `package | scene | line`. When a scene
is about that package, its own lines replace the general ones. On top of the rules above:

- Public technical history only, with a source in the pull request description.
- True whatever version is installed, today and in five years.
- If the package's maintainers ask for a line to go, it goes, without argument.

## Changing the code

A current stable Rust toolchain is all it takes. Before opening a pull request:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

CI also runs `cargo deny check`, `cargo shear`, `typos`, `taplo fmt --check`, `actionlint` and
`zizmor`.

A scene must never state something false. A new detector comes with tests at its boundaries, and
when the tool cannot be sure of a fact, the scene is left out.
