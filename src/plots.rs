//! Plot points: the true facts an episode is written from, one detector each.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Serialize;

use crate::date::Date;
use crate::model::{Project, major};
use crate::registry::RegistryFacts;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "facts", rename_all = "snake_case")]
pub(crate) enum Plot {
    Entourage {
        direct: usize,
        installed: usize,
    },
    AlmostReady {
        name: String,
        version: String,
    },
    Ghost {
        name: String,
        version: String,
        message: String,
        direct: bool,
    },
    Clones {
        name: String,
        versions: usize,
    },
    Feud {
        name: String,
        peer: String,
        range: String,
        installed: String,
    },
    TimeCapsule {
        name: String,
        version: String,
        latest: String,
        majors_behind: u64,
    },
    MissingPerson {
        name: String,
        since_year: i64,
        years: i64,
    },
    CollectionPlate {
        packages: usize,
    },
    NightVisitor {
        packages: usize,
        example: String,
    },
    NewArrival {
        name: String,
        version: String,
    },
    WrittenOut {
        name: String,
        version: String,
    },
    Recast {
        name: String,
        from: String,
        to: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Kind {
    Entourage,
    AlmostReady,
    Ghost,
    Clones,
    Feud,
    TimeCapsule,
    MissingPerson,
    CollectionPlate,
    NightVisitor,
    NewArrival,
    WrittenOut,
    Recast,
}

const ENTOURAGE_PER_DIRECT: usize = 20;
const CLONES_OF_ANYTHING: usize = 4;
const MISSING_AFTER_YEARS: i64 = 3;
const MESSAGE_LENGTH: usize = 110;

impl Plot {
    pub(crate) fn kind(&self) -> Kind {
        match self {
            Self::Entourage { .. } => Kind::Entourage,
            Self::AlmostReady { .. } => Kind::AlmostReady,
            Self::Ghost { .. } => Kind::Ghost,
            Self::Clones { .. } => Kind::Clones,
            Self::Feud { .. } => Kind::Feud,
            Self::TimeCapsule { .. } => Kind::TimeCapsule,
            Self::MissingPerson { .. } => Kind::MissingPerson,
            Self::CollectionPlate { .. } => Kind::CollectionPlate,
            Self::NightVisitor { .. } => Kind::NightVisitor,
            Self::NewArrival { .. } => Kind::NewArrival,
            Self::WrittenOut { .. } => Kind::WrittenOut,
            Self::Recast { .. } => Kind::Recast,
        }
    }

    /// The package the scene is about, when it is about one.
    pub(crate) fn name(&self) -> Option<&str> {
        match self {
            Self::AlmostReady { name, .. }
            | Self::Ghost { name, .. }
            | Self::Clones { name, .. }
            | Self::Feud { name, .. }
            | Self::TimeCapsule { name, .. }
            | Self::MissingPerson { name, .. }
            | Self::NewArrival { name, .. }
            | Self::WrittenOut { name, .. }
            | Self::Recast { name, .. } => Some(name),
            Self::Entourage { .. } | Self::CollectionPlate { .. } | Self::NightVisitor { .. } => {
                None
            }
        }
    }

    /// How much the scene deserves its place: the rarer and more specific the fact, the higher.
    pub(crate) fn juiciness(&self) -> u64 {
        match self {
            Self::NewArrival { .. } => 130,
            Self::Recast { .. } => 120,
            Self::WrittenOut { .. } => 110,
            Self::Feud { .. } => 100,
            Self::Ghost { direct: true, .. } => 90,
            Self::TimeCapsule { majors_behind, .. } => 70 + (*majors_behind).min(15),
            Self::MissingPerson { years, .. } => 65 + years.unsigned_abs().min(15),
            Self::Ghost { direct: false, .. } => 60,
            Self::Entourage { .. } => 55,
            Self::AlmostReady { .. } => 50,
            Self::Clones { versions, .. } => 40 + (*versions as u64).min(9),
            Self::CollectionPlate { .. } | Self::NightVisitor { .. } => 30,
        }
    }

    /// The values a line may quote, by placeholder name. `Kind::placeholders` lists the same names.
    pub(crate) fn values(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Entourage { direct, installed } => {
                vec![
                    ("guests", counted(direct, "guest")),
                    ("installed", installed.to_string()),
                ]
            }
            Self::AlmostReady { name, version }
            | Self::NewArrival { name, version }
            | Self::WrittenOut { name, version } => {
                vec![("name", name.clone()), ("version", version.clone())]
            }
            Self::Recast { name, from, to } => vec![
                ("name", name.clone()),
                ("from", from.clone()),
                ("to", to.clone()),
            ],
            Self::Ghost {
                name,
                version,
                message,
                ..
            } => vec![
                ("name", name.clone()),
                ("version", version.clone()),
                ("message", message.clone()),
            ],
            Self::Clones { name, versions } => {
                vec![("name", name.clone()), ("versions", versions.to_string())]
            }
            Self::Feud {
                name,
                peer,
                range,
                installed,
            } => vec![
                ("name", name.clone()),
                ("peer", peer.clone()),
                ("range", range.clone()),
                ("installed", installed.clone()),
            ],
            Self::TimeCapsule {
                name,
                version,
                latest,
                majors_behind,
            } => vec![
                ("name", name.clone()),
                ("version", version.clone()),
                ("latest", latest.clone()),
                ("majors", counted(majors_behind, "major version")),
            ],
            Self::MissingPerson {
                name,
                since_year,
                years,
            } => vec![
                ("name", name.clone()),
                ("year", since_year.to_string()),
                ("years", years.to_string()),
            ],
            Self::CollectionPlate { packages } => vec![("packages", counted(packages, "package"))],
            Self::NightVisitor { packages, example } => {
                vec![
                    ("packages", counted(packages, "package")),
                    ("name", example.clone()),
                ]
            }
        }
    }
}

impl Kind {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 12] = [
        Self::Entourage,
        Self::AlmostReady,
        Self::Ghost,
        Self::Clones,
        Self::Feud,
        Self::TimeCapsule,
        Self::MissingPerson,
        Self::CollectionPlate,
        Self::NightVisitor,
        Self::NewArrival,
        Self::WrittenOut,
        Self::Recast,
    ];

    /// The name a line file and the JSON output know the kind by.
    pub(crate) fn slug(self) -> &'static str {
        match self {
            Self::Entourage => "entourage",
            Self::AlmostReady => "almost_ready",
            Self::Ghost => "ghost",
            Self::Clones => "clones",
            Self::Feud => "feud",
            Self::TimeCapsule => "time_capsule",
            Self::MissingPerson => "missing_person",
            Self::CollectionPlate => "collection_plate",
            Self::NightVisitor => "night_visitor",
            Self::NewArrival => "new_arrival",
            Self::WrittenOut => "written_out",
            Self::Recast => "recast",
        }
    }

    /// True of nearly every project, so an episode gets one such scene at most.
    pub(crate) fn is_ambient(self) -> bool {
        matches!(self, Self::CollectionPlate | Self::NightVisitor)
    }

    /// What the change under review did, told after everything that was already true.
    pub(crate) fn is_tonight(self) -> bool {
        matches!(self, Self::NewArrival | Self::WrittenOut | Self::Recast)
    }

    #[cfg(test)]
    pub(crate) fn placeholders(self) -> &'static [&'static str] {
        match self {
            Self::Entourage => &["guests", "installed"],
            Self::AlmostReady | Self::NewArrival | Self::WrittenOut => &["name", "version"],
            Self::Recast => &["name", "from", "to"],
            Self::Ghost => &["name", "version", "message"],
            Self::Clones => &["name", "versions"],
            Self::Feud => &["name", "peer", "range", "installed"],
            Self::TimeCapsule => &["name", "version", "latest", "majors"],
            Self::MissingPerson => &["name", "year", "years"],
            Self::CollectionPlate => &["packages"],
            Self::NightVisitor => &["packages", "name"],
        }
    }
}

fn counted<N: std::fmt::Display + PartialEq + From<u8>>(count: &N, noun: &str) -> String {
    if *count == N::from(1) {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

pub(crate) fn offline(project: &Project) -> Vec<Plot> {
    let mut plots = Vec::new();
    plots.extend(entourage(project));
    plots.extend(almost_ready(project));
    plots.extend(ghosts(project));
    plots.extend(clones(project));
    plots.extend(feuds(project));
    plots.extend(collection_plate(project));
    plots.extend(night_visitor(project));
    plots
}

pub(crate) fn from_registry(
    project: &Project,
    facts: &HashMap<String, RegistryFacts>,
    today: Date,
) -> Vec<Plot> {
    let mut plots = Vec::new();
    for package in project.direct() {
        let Some(facts) = facts.get(&package.name) else {
            continue;
        };
        plots.extend(time_capsule(&package.name, &package.version, &facts.latest));
        plots.extend(missing_person(&package.name, &facts.modified, today));
    }
    plots
}

/// What changed among the direct dependencies between two states of one project.
pub(crate) fn since(before: &Project, now: &Project) -> Vec<Plot> {
    let before = direct_versions(before);
    let now = direct_versions(now);
    let arrived_or_recast = now.iter().filter_map(|(name, version)| {
        before.get(name).map_or_else(
            || {
                Some(Plot::NewArrival {
                    name: (*name).to_owned(),
                    version: (*version).to_owned(),
                })
            },
            |earlier| recast(name, earlier, version),
        )
    });
    let written_out = before
        .iter()
        .filter(|(name, _)| !now.contains_key(*name))
        .map(|(name, version)| Plot::WrittenOut {
            name: (*name).to_owned(),
            version: (*version).to_owned(),
        });
    arrived_or_recast.chain(written_out).collect()
}

fn direct_versions(project: &Project) -> BTreeMap<&str, &str> {
    project
        .direct()
        .map(|package| (package.name.as_str(), package.version.as_str()))
        .collect()
}

fn recast(name: &str, from: &str, to: &str) -> Option<Plot> {
    (major(to)? > major(from)?).then(|| Plot::Recast {
        name: name.to_owned(),
        from: from.to_owned(),
        to: to.to_owned(),
    })
}

fn entourage(project: &Project) -> Option<Plot> {
    let direct = project.direct().count();
    let installed = project.packages.len();
    (direct > 0 && installed > direct * ENTOURAGE_PER_DIRECT)
        .then_some(Plot::Entourage { direct, installed })
}

fn almost_ready(project: &Project) -> Vec<Plot> {
    project
        .direct()
        .filter(|package| major(&package.version) == Some(0))
        .map(|package| Plot::AlmostReady {
            name: package.name.clone(),
            version: package.version.clone(),
        })
        .collect()
}

fn ghosts(project: &Project) -> Vec<Plot> {
    let mut deprecated: Vec<_> = project
        .packages
        .iter()
        .filter(|package| package.deprecation.is_some())
        .collect();
    deprecated.sort_by_key(|package| !package.direct);
    let mut seen = BTreeSet::new();
    deprecated
        .into_iter()
        .filter(|package| seen.insert(&package.name))
        .map(|package| Plot::Ghost {
            name: package.name.clone(),
            version: package.version.clone(),
            message: quotable(package.deprecation.as_deref().unwrap_or_default()),
            direct: package.direct,
        })
        .collect()
}

fn quotable(message: &str) -> String {
    let flat = message
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('"', "'");
    if flat.is_empty() {
        return "no reason given".to_owned();
    }
    if flat.chars().count() <= MESSAGE_LENGTH {
        return flat;
    }
    // One character past the limit, so a sentence or a word that ends exactly at it still counts.
    let window: String = flat.chars().take(MESSAGE_LENGTH + 1).collect();
    if let Some(end) = last_sentence_end(&window) {
        return window[..end].to_owned();
    }
    let cut: String = match window.rfind(' ') {
        Some(space) => window[..space].to_owned(),
        None => window.chars().take(MESSAGE_LENGTH).collect(),
    };
    format!("{cut}…")
}

/// Where the last sentence that ends inside `text`, followed by a space, stops.
fn last_sentence_end(text: &str) -> Option<usize> {
    [". ", "! ", "? "]
        .iter()
        .flat_map(|end| text.match_indices(end))
        .map(|(at, _)| at + 1)
        .max()
}

fn clones(project: &Project) -> Vec<Plot> {
    let mut copies: BTreeMap<&str, (BTreeSet<&str>, bool)> = BTreeMap::new();
    for package in &project.packages {
        let (versions, household_name) = copies.entry(&package.name).or_default();
        versions.insert(&package.version);
        *household_name |= package.household_name;
    }
    copies
        .into_iter()
        .filter(|(_, (versions, household_name))| {
            versions.len()
                >= if *household_name {
                    2
                } else {
                    CLONES_OF_ANYTHING
                }
        })
        .map(|(name, (versions, _))| Plot::Clones {
            name: name.to_owned(),
            versions: versions.len(),
        })
        .collect()
}

fn feuds(project: &Project) -> Vec<Plot> {
    let mut seen = BTreeSet::new();
    project
        .packages
        .iter()
        .flat_map(|package| package.peers.iter().map(move |peer| (package, peer)))
        .filter(|(package, peer)| !peer.satisfied && seen.insert((&package.name, &peer.name)))
        .map(|(package, peer)| Plot::Feud {
            name: package.name.clone(),
            peer: peer.name.clone(),
            range: peer.range.clone(),
            installed: peer.installed.clone(),
        })
        .collect()
}

fn collection_plate(project: &Project) -> Option<Plot> {
    let packages = project
        .packages
        .iter()
        .filter(|package| package.asks_for_funding)
        .count();
    (packages > 0).then_some(Plot::CollectionPlate { packages })
}

fn night_visitor(project: &Project) -> Option<Plot> {
    let visitors: BTreeSet<_> = project
        .packages
        .iter()
        .filter(|package| package.runs_install_script)
        .map(|package| (!package.direct, package.name.as_str()))
        .collect();
    let (_, example) = visitors.first()?;
    let names: BTreeSet<_> = visitors.iter().map(|(_, name)| name).collect();
    Some(Plot::NightVisitor {
        packages: names.len(),
        example: (*example).to_owned(),
    })
}

fn time_capsule(name: &str, version: &str, latest: &str) -> Option<Plot> {
    let majors_behind = major(latest)?.checked_sub(major(version)?)?;
    (majors_behind > 0).then(|| Plot::TimeCapsule {
        name: name.to_owned(),
        version: version.to_owned(),
        latest: latest.to_owned(),
        majors_behind,
    })
}

fn missing_person(name: &str, modified: &str, today: Date) -> Option<Plot> {
    let modified = Date::from_timestamp(modified)?;
    let years = modified.whole_years_until(today);
    (years >= MISSING_AFTER_YEARS).then(|| Plot::MissingPerson {
        name: name.to_owned(),
        since_year: modified.year,
        years,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{Kind, Plot, from_registry, offline, since};
    use crate::date::date;
    use crate::model::{Package, Peer, Project, package};
    use crate::registry::RegistryFacts;

    fn project(packages: Vec<Package>) -> Project {
        Project {
            name: "shop-web".to_owned(),
            packages,
        }
    }

    fn direct(name: &str, version: &str) -> Package {
        Package {
            direct: true,
            ..package(name, version)
        }
    }

    fn ghost_message(message: &str) -> String {
        let package = Package {
            deprecation: Some(message.to_owned()),
            ..package("old", "1.0.0")
        };
        match offline(&project(vec![package])).remove(0) {
            Plot::Ghost { message, .. } => message,
            other => panic!("expected a ghost, got {other:?}"),
        }
    }

    fn crowd(count: usize) -> Vec<Package> {
        (0..count)
            .map(|index| package(&format!("extra-{index}"), "1.0.0"))
            .collect()
    }

    #[test]
    fn an_empty_project_has_no_plot() {
        assert_eq!(offline(&project(Vec::new())), []);
    }

    #[test]
    fn a_calm_project_has_no_plot() {
        let plots = offline(&project(vec![
            direct("ms", "2.1.3"),
            package("tiny", "1.0.0"),
        ]));

        assert_eq!(plots, []);
    }

    #[test]
    fn an_entourage_needs_more_than_twenty_packages_per_direct_dependency() {
        let with = |extras| {
            let mut packages = vec![direct("webpack", "5.0.0")];
            packages.extend(crowd(extras));
            offline(&project(packages))
        };

        assert_eq!(with(19), []);
        assert_eq!(
            with(20),
            [Plot::Entourage {
                direct: 1,
                installed: 21
            }]
        );
    }

    #[test]
    fn only_direct_dependencies_at_zero_are_almost_ready() {
        let plots = offline(&project(vec![
            direct("axios", "0.21.4"),
            direct("esbuild", "0.19.12"),
            direct("lodash", "4.17.21"),
            package("deep", "0.1.0"),
        ]));

        assert_eq!(
            plots,
            [
                Plot::AlmostReady {
                    name: "axios".to_owned(),
                    version: "0.21.4".to_owned()
                },
                Plot::AlmostReady {
                    name: "esbuild".to_owned(),
                    version: "0.19.12".to_owned()
                },
            ]
        );
    }

    #[test]
    fn a_deprecated_package_is_a_ghost_once_however_many_copies_exist() {
        let deprecated = |version: &str| Package {
            deprecation: Some("Please  use\n\"other\" instead".to_owned()),
            ..package("old", version)
        };
        let plots = offline(&project(vec![deprecated("1.0.0"), deprecated("2.0.0")]));

        assert_eq!(
            plots,
            [Plot::Ghost {
                name: "old".to_owned(),
                version: "1.0.0".to_owned(),
                message: "Please use 'other' instead".to_owned(),
                direct: false,
            }]
        );
    }

    #[test]
    fn a_long_or_missing_deprecation_message_stays_quotable() {
        assert_eq!(ghost_message(""), "no reason given");
        assert_eq!(ghost_message(&"x".repeat(110)).chars().count(), 110);
        assert_eq!(
            ghost_message(&"x".repeat(111)),
            format!("{}…", "x".repeat(110))
        );
    }

    #[test]
    fn a_long_deprecation_message_stops_at_a_sentence_that_fits() {
        assert_eq!(
            ghost_message(
                "uuid@10 and below is no longer supported. For ESM codebases, update to uuid@latest. For CommonJS codebases, use uuid@11 (but be aware this version will likely be deprecated in 2028)."
            ),
            "uuid@10 and below is no longer supported. For ESM codebases, update to uuid@latest."
        );
        assert_eq!(
            ghost_message(
                "This is a stub types definition. dompurify provides its own type definitions, so you do not need this installed."
            ),
            "This is a stub types definition."
        );
    }

    #[test]
    fn a_long_message_without_a_sentence_that_fits_is_cut_between_words() {
        assert_eq!(
            ghost_message(&"word ".repeat(30)),
            format!("{}…", "word ".repeat(22).trim_end())
        );
    }

    #[test]
    fn a_sentence_or_a_word_ending_exactly_at_the_limit_is_kept_whole() {
        let sentence = format!("{}.", "x".repeat(109));
        let word = "x".repeat(110);

        assert_eq!(ghost_message(&format!("{sentence} More.")), sentence);
        assert_eq!(ghost_message(&format!("{word} more")), format!("{word}…"));
    }

    #[test]
    fn clones_need_four_versions_or_a_household_name() {
        let copies = |household_name, count: usize| -> Vec<Package> {
            (0..count)
                .map(|index| Package {
                    household_name,
                    ..package("ms", &format!("{index}.0.0"))
                })
                .collect()
        };
        let plots = |household_name, count| offline(&project(copies(household_name, count)));
        let clones = |versions| {
            [Plot::Clones {
                name: "ms".to_owned(),
                versions,
            }]
        };

        assert_eq!(plots(false, 3), []);
        assert_eq!(plots(false, 4), clones(4));
        assert_eq!(plots(true, 1), []);
        assert_eq!(plots(true, 2), clones(2));
    }

    #[test]
    fn a_direct_ghost_is_told_from_its_direct_copy_not_a_nested_one() {
        let deprecated = |version: &str, direct| Package {
            direct,
            deprecation: Some(format!("{version} is no longer supported")),
            ..package("request", version)
        };
        let plots = offline(&project(vec![
            deprecated("2.80.0", false),
            deprecated("2.88.2", true),
        ]));

        assert_eq!(
            plots,
            [Plot::Ghost {
                name: "request".to_owned(),
                version: "2.88.2".to_owned(),
                message: "2.88.2 is no longer supported".to_owned(),
                direct: true,
            }]
        );
    }

    #[test]
    fn the_same_version_installed_twice_is_not_a_clone() {
        let packages = (0..5).map(|_| package("ms", "2.1.3")).collect();

        assert_eq!(offline(&project(packages)), []);
    }

    #[test]
    fn an_unsatisfied_peer_is_a_feud_and_a_satisfied_one_is_not() {
        let peer = |name: &str, satisfied| Peer {
            name: name.to_owned(),
            range: "^18.0.0".to_owned(),
            installed: "16.14.0".to_owned(),
            satisfied,
        };
        let widget = Package {
            peers: vec![peer("react", false), peer("react-dom", true)],
            ..package("widget", "1.0.0")
        };
        let plots = offline(&project(vec![widget.clone(), widget]));

        assert_eq!(
            plots,
            [Plot::Feud {
                name: "widget".to_owned(),
                peer: "react".to_owned(),
                range: "^18.0.0".to_owned(),
                installed: "16.14.0".to_owned(),
            }]
        );
    }

    #[test]
    fn funding_requests_are_counted_into_one_collection_plate() {
        let asking = |name: &str| Package {
            asks_for_funding: true,
            ..package(name, "1.0.0")
        };
        let plots = offline(&project(vec![
            asking("a"),
            asking("b"),
            package("c", "1.0.0"),
        ]));

        assert_eq!(plots, [Plot::CollectionPlate { packages: 2 }]);
    }

    #[test]
    fn the_night_visitor_names_a_direct_dependency_when_one_runs_a_script() {
        let script = |package: Package| Package {
            runs_install_script: true,
            ..package
        };
        let plots = offline(&project(vec![
            script(package("aaa-transitive", "1.0.0")),
            script(direct("esbuild", "1.0.0")),
            script(package("fsevents", "1.2.13")),
            script(package("fsevents", "2.3.3")),
        ]));

        assert_eq!(
            plots,
            [Plot::NightVisitor {
                packages: 3,
                example: "esbuild".to_owned()
            }]
        );
    }

    fn facts(entries: &[(&str, &str, &str)]) -> HashMap<String, RegistryFacts> {
        entries
            .iter()
            .map(|(name, latest, modified)| {
                let facts = RegistryFacts {
                    latest: (*latest).to_owned(),
                    modified: (*modified).to_owned(),
                };
                ((*name).to_owned(), facts)
            })
            .collect()
    }

    #[test]
    fn nothing_from_the_registry_means_no_registry_plot() {
        let project = project(vec![direct("webpack", "4.46.0")]);

        assert_eq!(
            from_registry(&project, &HashMap::new(), date(2026, 11, 13)),
            []
        );
    }

    #[test]
    fn a_direct_dependency_majors_behind_is_a_time_capsule() {
        let project = project(vec![
            direct("webpack", "4.46.0"),
            direct("lodash", "4.17.20"),
            direct("canary", "9.0.0-next.1"),
        ]);
        let facts = facts(&[
            ("webpack", "5.99.0", "2026-11-01T00:00:00.000Z"),
            ("lodash", "4.17.21", "2026-11-01T00:00:00.000Z"),
            ("canary", "8.2.0", "2026-11-01T00:00:00.000Z"),
        ]);

        assert_eq!(
            from_registry(&project, &facts, date(2026, 11, 13)),
            [Plot::TimeCapsule {
                name: "webpack".to_owned(),
                version: "4.46.0".to_owned(),
                latest: "5.99.0".to_owned(),
                majors_behind: 1,
            }]
        );
    }

    #[test]
    fn a_package_is_missing_only_after_three_whole_years() {
        let project = project(vec![direct("left-pad", "1.3.0")]);
        let on = |modified: &str| {
            from_registry(
                &project,
                &facts(&[("left-pad", "1.3.0", modified)]),
                date(2026, 11, 13),
            )
        };

        assert_eq!(on("2023-11-14T09:00:00.000Z"), []);
        assert_eq!(
            on("2023-11-13T09:00:00.000Z"),
            [Plot::MissingPerson {
                name: "left-pad".to_owned(),
                since_year: 2023,
                years: 3
            }]
        );
        assert_eq!(on("not a date"), []);
    }

    #[test]
    fn transitive_packages_get_no_registry_plot() {
        let project = project(vec![package("webpack", "4.46.0")]);
        let facts = facts(&[("webpack", "5.99.0", "2019-01-01T00:00:00.000Z")]);

        assert_eq!(from_registry(&project, &facts, date(2026, 11, 13)), []);
    }

    #[test]
    fn identical_states_have_nothing_new_to_tell() {
        let state = || {
            project(vec![
                direct("express", "4.18.2"),
                package("accepts", "1.3.8"),
            ])
        };

        assert_eq!(since(&state(), &state()), []);
        assert_eq!(since(&project(Vec::new()), &project(Vec::new())), []);
    }

    #[test]
    fn a_direct_dependency_that_was_not_there_before_is_a_new_arrival() {
        let before = project(vec![direct("express", "4.18.2")]);
        let now = project(vec![
            direct("express", "4.18.2"),
            direct("left-pad", "1.3.0"),
        ]);

        assert_eq!(
            since(&before, &now),
            [Plot::NewArrival {
                name: "left-pad".to_owned(),
                version: "1.3.0".to_owned(),
            }]
        );
    }

    #[test]
    fn a_direct_dependency_that_is_no_longer_there_is_written_out() {
        let before = project(vec![
            direct("express", "4.18.2"),
            direct("left-pad", "1.3.0"),
        ]);
        let now = project(vec![direct("express", "4.18.2")]);

        assert_eq!(
            since(&before, &now),
            [Plot::WrittenOut {
                name: "left-pad".to_owned(),
                version: "1.3.0".to_owned(),
            }]
        );
    }

    #[test]
    fn only_an_upgrade_across_a_major_version_is_a_recast() {
        let changed = |from: &str, to: &str| {
            since(
                &project(vec![direct("express", from)]),
                &project(vec![direct("express", to)]),
            )
        };
        let recast = |from: &str, to: &str| {
            [Plot::Recast {
                name: "express".to_owned(),
                from: from.to_owned(),
                to: to.to_owned(),
            }]
        };

        assert_eq!(changed("4.18.2", "5.0.0"), recast("4.18.2", "5.0.0"));
        assert_eq!(changed("0.9.0", "1.0.0"), recast("0.9.0", "1.0.0"));
        assert_eq!(changed("4.18.2", "4.99.0"), []);
        assert_eq!(changed("5.0.0", "4.18.2"), []);
        assert_eq!(changed("github:me/fork", "5.0.0"), []);
    }

    #[test]
    fn transitive_packages_coming_and_going_are_not_news() {
        let before = project(vec![direct("express", "4.18.2"), package("qs", "6.11.0")]);
        let now = project(vec![direct("express", "4.18.2"), package("ms", "2.1.3")]);

        assert_eq!(since(&before, &now), []);
    }

    #[test]
    fn many_changes_are_all_reported() {
        let before = project(vec![
            direct("express", "4.18.2"),
            direct("request", "2.88.2"),
            direct("moment", "2.30.1"),
        ]);
        let now = project(vec![
            direct("express", "5.1.0"),
            direct("axios", "1.7.0"),
            direct("dayjs", "1.11.0"),
        ]);

        let kinds: Vec<_> = since(&before, &now).iter().map(Plot::kind).collect();

        assert_eq!(
            kinds,
            [
                Kind::NewArrival,
                Kind::NewArrival,
                Kind::Recast,
                Kind::WrittenOut,
                Kind::WrittenOut
            ]
        );
    }

    #[test]
    fn every_plot_offers_exactly_the_placeholders_its_kind_declares() {
        let plots = [
            Plot::Entourage {
                direct: 1,
                installed: 30,
            },
            Plot::AlmostReady {
                name: String::new(),
                version: String::new(),
            },
            Plot::Ghost {
                name: String::new(),
                version: String::new(),
                message: String::new(),
                direct: true,
            },
            Plot::Clones {
                name: String::new(),
                versions: 4,
            },
            Plot::Feud {
                name: String::new(),
                peer: String::new(),
                range: String::new(),
                installed: String::new(),
            },
            Plot::TimeCapsule {
                name: String::new(),
                version: String::new(),
                latest: String::new(),
                majors_behind: 2,
            },
            Plot::MissingPerson {
                name: String::new(),
                since_year: 2020,
                years: 6,
            },
            Plot::CollectionPlate { packages: 1 },
            Plot::NightVisitor {
                packages: 1,
                example: String::new(),
            },
            Plot::NewArrival {
                name: String::new(),
                version: String::new(),
            },
            Plot::WrittenOut {
                name: String::new(),
                version: String::new(),
            },
            Plot::Recast {
                name: String::new(),
                from: String::new(),
                to: String::new(),
            },
        ];

        assert_eq!(plots.len(), Kind::ALL.len());
        for plot in plots {
            let written = serde_json::to_value(&plot).expect("serializes");
            assert_eq!(written["kind"], plot.kind().slug());
            let offered: Vec<_> = plot.values().into_iter().map(|(name, _)| name).collect();
            assert_eq!(offered, plot.kind().placeholders(), "{:?}", plot.kind());
        }
    }

    #[test]
    fn counts_read_naturally_for_one_and_for_many() {
        let values = |plot: Plot| plot.values();

        assert_eq!(
            values(Plot::CollectionPlate { packages: 1 })[0].1,
            "1 package"
        );
        assert_eq!(
            values(Plot::CollectionPlate { packages: 46 })[0].1,
            "46 packages"
        );
    }
}
