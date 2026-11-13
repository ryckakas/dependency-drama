//! Plot points: the true facts an episode is written from, one detector each.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::date::Date;
use crate::model::{Project, major};
use crate::registry::RegistryFacts;

#[derive(Debug, Clone, PartialEq, Eq)]
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
        }
    }

    /// How much the scene deserves its place: the rarer and more specific the fact, the higher.
    pub(crate) fn juiciness(&self) -> u64 {
        match self {
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
            Self::AlmostReady { name, version } => {
                vec![("name", name.clone()), ("version", version.clone())]
            }
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
    pub(crate) const ALL: [Self; 9] = [
        Self::Entourage,
        Self::AlmostReady,
        Self::Ghost,
        Self::Clones,
        Self::Feud,
        Self::TimeCapsule,
        Self::MissingPerson,
        Self::CollectionPlate,
        Self::NightVisitor,
    ];

    /// True of nearly every project, so an episode gets one such scene at most.
    pub(crate) fn is_ambient(self) -> bool {
        matches!(self, Self::CollectionPlate | Self::NightVisitor)
    }

    #[cfg(test)]
    pub(crate) fn placeholders(self) -> &'static [&'static str] {
        match self {
            Self::Entourage => &["guests", "installed"],
            Self::AlmostReady => &["name", "version"],
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
    let cut: String = flat.chars().take(MESSAGE_LENGTH).collect();
    format!("{}…", cut.trim_end())
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

    use super::{Kind, Plot, from_registry, offline};
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
        let ghost = |message: &str| {
            let package = Package {
                deprecation: Some(message.to_owned()),
                ..package("old", "1.0.0")
            };
            match offline(&project(vec![package])).remove(0) {
                Plot::Ghost { message, .. } => message,
                other => panic!("expected a ghost, got {other:?}"),
            }
        };

        assert_eq!(ghost(""), "no reason given");
        assert_eq!(ghost(&"x".repeat(110)).chars().count(), 110);
        assert_eq!(ghost(&"x".repeat(111)).chars().count(), 111);
        assert!(ghost(&"x".repeat(111)).ends_with('…'));
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
        ];

        assert_eq!(plots.len(), Kind::ALL.len());
        for plot in plots {
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
