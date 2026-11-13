//! Dependency Drama: a project's real dependency facts, narrated as a soap opera episode.

mod date;
mod episode;
mod model;
mod npm;
mod plots;
mod registry;
mod render;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use crate::date::Date;
use crate::npm::ReadError;
use crate::registry::{HttpRegistry, Registry};

/// Your dependencies, narrated as a soap opera. Every line is a true fact from the lockfile.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Arguments {
    /// The project directory, or its package-lock.json.
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Write Markdown, for a pull request comment or a job summary.
    #[arg(long)]
    markdown: bool,

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
        Err(error @ ReadError::TooOld) => print(&format!("{error}\n")),
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
    let project = npm::read(&arguments.path)?;
    let mut plots = plots::offline(&project);
    if let Some(registry) = registry {
        let facts = registry::lookup_direct(&project, registry);
        plots.extend(plots::from_registry(&project, &facts, today));
    }
    let episode = episode::write(&project.name, plots);
    Ok(if arguments.markdown {
        render::markdown(&episode)
    } else {
        render::terminal(&episode)
    })
}

/// A closed pipe (`| head`) ends the output quietly; `println!` would panic on it.
fn print(output: &str) -> ExitCode {
    let _written: io::Result<()> = io::stdout().lock().write_all(output.as_bytes());
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

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
    fn markdown_is_chosen_by_the_flag() {
        let output = run(&arguments(true), None, date(2026, 11, 13)).expect("episode");

        assert!(output.starts_with("### 📺 Previously on `shop-web`…"));
    }
}
