//! Dependency Drama: a project's real dependency facts, told as deadpan jokes.

mod date;
mod episode;
mod lockfile;
mod model;
mod npm;
mod npmrc;
mod plots;
mod pnpm;
mod registry;
mod render;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use std::collections::HashMap;

use crate::date::Date;
use crate::episode::Episode;
use crate::lockfile::ReadError;
use crate::model::Project;
use crate::plots::Plot;
use crate::registry::{HttpRegistry, Registry, RegistryFacts};

/// Deadpan jokes about your dependencies. Every line is a true fact from the lockfile.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Arguments {
    /// The project directory, or its package-lock.json or pnpm-lock.yaml.
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Write Markdown, for a pull request comment or a job summary.
    #[arg(long)]
    markdown: bool,

    /// Write JSON: each scene with its kind, the facts behind it, and its line.
    #[arg(long, conflicts_with = "markdown")]
    json: bool,

    /// The same project before a change: a directory, or its lockfile.
    ///
    /// Direct dependencies that were added, removed or moved up a major version since then are
    /// told as tonight's scenes.
    #[arg(long, value_name = "PATH")]
    base: Option<PathBuf>,

    /// Ask the npm registry nothing.
    ///
    /// Without this, the names of direct dependencies installed from registry.npmjs.org are
    /// looked up there, to learn how far behind they are. Nothing else leaves the machine.
    #[arg(long)]
    offline: bool,
}

fn main() -> ExitCode {
    let arguments = Arguments::parse();
    let registry = (!arguments.offline).then(HttpRegistry::npm);
    let registry: Option<&dyn Registry> = match &registry {
        Some(registry) => Some(registry),
        None => None,
    };
    match run(&arguments, registry, Date::today()) {
        Ok(output) => print(&output),
        Err(error) if error.is_an_old_lockfile() => print(&format!("{error}\n")),
        Err(error) => {
            let _written: io::Result<()> =
                writeln!(io::stderr().lock(), "dependency-drama: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(
    arguments: &Arguments,
    registry: Option<&dyn Registry>,
    today: Date,
) -> Result<String, ReadError> {
    let project = lockfile::read(&arguments.path)?;
    let facts = registry.map(|registry| registry::lookup_direct(&project, registry));
    let mut plots = already_true(&project, facts.as_ref(), today);
    let mut rerun = false;
    if let Some(base) = &arguments.base {
        let before = lockfile::read(base)?;
        // Told under today's name, so only the facts can make the two differ.
        rerun = episode::write(&project.name, plots.clone()).scenes
            == episode::write(&project.name, already_true(&before, facts.as_ref(), today)).scenes;
        plots.extend(plots::since(&before, &project));
    }
    let episode = Episode {
        rerun,
        ..episode::write(&project.name, plots)
    };
    Ok(if arguments.json {
        render::json(&episode)
    } else if arguments.markdown {
        render::markdown(&episode)
    } else {
        render::terminal(&episode)
    })
}

/// Everything but tonight's scenes. The registry was asked about today's direct dependencies only,
/// so a base is told without facts for a dependency the change removed.
fn already_true(
    project: &Project,
    facts: Option<&HashMap<String, RegistryFacts>>,
    today: Date,
) -> Vec<Plot> {
    let mut plots = plots::offline(project);
    if let Some(facts) = facts {
        plots.extend(plots::from_registry(project, facts, today));
    }
    plots
}

/// A closed pipe (`| head`) ends the output quietly; `println!` would panic on it.
fn print(output: &str) -> ExitCode {
    let _written: io::Result<()> = io::stdout().lock().write_all(output.as_bytes());
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::Parser;

    use super::{Arguments, run};
    use crate::date::date;
    use crate::registry::{Registry, RegistryFacts};

    struct Stub;

    impl Registry for Stub {
        fn lookup(&self, name: &str) -> Option<RegistryFacts> {
            (name == "webpack").then(|| RegistryFacts {
                latest: "5.99.0".to_owned(),
                modified: "2026-11-01T00:00:00.000Z".to_owned(),
            })
        }
    }

    struct Unplugged;

    impl Registry for Unplugged {
        fn lookup(&self, _name: &str) -> Option<RegistryFacts> {
            None
        }
    }

    fn arguments(markdown: bool) -> Arguments {
        Arguments {
            path: PathBuf::from("tests/fixtures/shop-web"),
            markdown,
            json: false,
            base: None,
            offline: false,
        }
    }

    #[test]
    fn a_real_project_gets_a_full_episode() {
        let output = run(&arguments(false), None, date(2026, 11, 13)).expect("episode");

        assert!(output.starts_with("PREVIOUSLY ON shop-web…\n\n"));
        assert_eq!(output.lines().count(), 7, "{output}");
        assert!(!output.contains('{'), "{output}");
    }

    #[test]
    fn the_registry_adds_what_the_lockfile_cannot_know() {
        let output = run(&arguments(false), Some(&Stub), date(2026, 11, 13)).expect("episode");

        assert!(
            output.contains("webpack") && output.contains("5.99.0"),
            "{output}"
        );
    }

    #[test]
    fn a_registry_that_answers_nothing_still_leaves_the_offline_episode() {
        let today = date(2026, 11, 13);
        let offline = run(&arguments(false), None, today).expect("episode");
        let unplugged = run(&arguments(false), Some(&Unplugged), today).expect("episode");

        assert_eq!(offline, unplugged);
    }

    #[test]
    fn json_is_chosen_by_the_flag_and_tells_the_same_episode_as_the_terminal() {
        let today = date(2026, 11, 13);
        let terminal = run(&arguments(false), None, today).expect("episode");
        let json = Arguments {
            json: true,
            ..arguments(false)
        };

        let output = run(&json, None, today).expect("episode");

        let document: serde_json::Value = serde_json::from_str(&output).expect("valid JSON");
        let scenes = document["scenes"].as_array().expect("scenes");
        assert_eq!(document["project"], "shop-web");
        assert_eq!(scenes.len(), 5);
        for scene in scenes {
            let text = scene["text"].as_str().expect("text");
            assert!(terminal.contains(text), "{text}");
            assert!(scene["kind"].is_string() && scene["facts"].is_object());
        }
    }

    #[test]
    fn json_and_markdown_cannot_be_asked_for_together() {
        let parsed = Arguments::try_parse_from(["dependency-drama", "--json", "--markdown"]);

        assert!(parsed.is_err());
    }

    fn lockfile(directory: &std::path::Path, packages: &str) -> PathBuf {
        std::fs::create_dir_all(directory).expect("directory");
        let text = format!(r#"{{"name":"shop","lockfileVersion":3,"packages":{{{packages}}}}}"#);
        std::fs::write(directory.join("package-lock.json"), text).expect("lockfile");
        directory.to_path_buf()
    }

    #[test]
    fn a_base_lockfile_adds_what_changed_since_as_tonights_scenes() {
        let scratch = tempfile::tempdir().expect("scratch directory");
        let base = lockfile(
            &scratch.path().join("base"),
            r#""":{"dependencies":{"express":"^4","request":"^2"}},
               "node_modules/express":{"version":"4.18.2"},
               "node_modules/request":{"version":"2.88.2"}"#,
        );
        let head = lockfile(
            &scratch.path().join("head"),
            r#""":{"dependencies":{"express":"^5","axios":"^1"}},
               "node_modules/express":{"version":"5.1.0"},
               "node_modules/axios":{"version":"1.7.0"}"#,
        );
        let arguments = Arguments {
            path: head,
            base: Some(base),
            ..arguments(false)
        };

        let output = run(&arguments, None, date(2026, 11, 13)).expect("episode");

        let (_, tonight) = output.split_once("\nTONIGHT…\n\n").expect("a second act");
        assert_eq!(tonight.lines().count(), 3, "{output}");
        assert!(
            tonight.contains("axios") && tonight.contains("1.7.0"),
            "{output}"
        );
        assert!(
            tonight.contains("express") && tonight.contains("5.1.0"),
            "{output}"
        );
        assert!(tonight.contains("request"), "{output}");
    }

    /// The Markdown for a project whose root gained one direct dependency, `extra`.
    fn markdown_after_adding(extra: &str) -> String {
        let scratch = tempfile::tempdir().expect("scratch directory");
        let shared = r#""node_modules/a":{"version":"0.1.0"},"node_modules/b":{"version":"0.2.0"}"#;
        let base = lockfile(
            &scratch.path().join("base"),
            &format!(r#""":{{"dependencies":{{"a":"*","b":"*"}}}},{shared}"#),
        );
        let head = lockfile(
            &scratch.path().join("head"),
            &format!(
                r#""":{{"dependencies":{{"a":"*","b":"*","extra":"*"}}}},{shared},"node_modules/extra":{extra}"#
            ),
        );
        let arguments = Arguments {
            path: head,
            base: Some(base),
            ..arguments(true)
        };
        run(&arguments, None, date(2026, 11, 13)).expect("episode")
    }

    #[test]
    fn a_change_that_leaves_previously_on_alone_gets_it_folded() {
        let output = markdown_after_adding(r#"{"version":"1.0.0"}"#);

        assert!(output.contains("<details>"), "{output}");
        assert!(output.contains("**Tonight…**"), "{output}");
    }

    #[test]
    fn a_change_that_adds_to_previously_on_shows_it_open() {
        let output = markdown_after_adding(r#"{"version":"1.0.0","deprecated":"Gone."}"#);

        assert!(!output.contains("<details>"), "{output}");
        assert!(output.contains("Gone."), "{output}");
    }

    #[test]
    fn a_base_identical_to_the_project_changes_nothing() {
        let today = date(2026, 11, 13);
        let compared = Arguments {
            base: Some(PathBuf::from("tests/fixtures/shop-web")),
            ..arguments(false)
        };

        assert_eq!(
            run(&compared, None, today).expect("episode"),
            run(&arguments(false), None, today).expect("episode")
        );
    }

    #[test]
    fn a_base_without_a_lockfile_is_an_error_not_an_episode() {
        let scratch = tempfile::tempdir().expect("scratch directory");
        let compared = Arguments {
            base: Some(scratch.path().to_path_buf()),
            ..arguments(false)
        };

        assert!(run(&compared, None, date(2026, 11, 13)).is_err());
    }

    #[test]
    fn a_pnpm_monorepo_gets_an_episode_from_what_its_members_ask_for() {
        let arguments = Arguments {
            path: PathBuf::from("tests/fixtures/pnpm-monorepo"),
            ..arguments(false)
        };

        let output = run(&arguments, None, date(2026, 11, 13)).expect("episode");

        assert!(
            output.starts_with("PREVIOUSLY ON pnpm-monorepo…\n\n"),
            "{output}"
        );
        assert!(
            output.contains("widget") && output.contains("^17.0.0"),
            "{output}"
        );
        assert!(output.contains("lodash"), "{output}");
    }

    #[test]
    fn markdown_is_chosen_by_the_flag() {
        let output = run(&arguments(true), None, date(2026, 11, 13)).expect("episode");

        assert!(output.starts_with("### 📺 Previously on `shop-web`…"));
    }
}
