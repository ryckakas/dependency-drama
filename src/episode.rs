//! Turns plot points into an episode: which scenes make the cut, and which line each one gets.

use std::cmp::Reverse;

use crate::plots::{Kind, Plot};

const SCENES: usize = 5;
/// Without this, a project with ten deprecated packages gets an episode about nothing else.
const SCENES_PER_KIND: usize = 2;
/// Scenes about the change under review come on top of the five, so they never push the rest out.
const SCENES_TONIGHT: usize = 3;
/// Changes named in the closing line before the rest are only counted.
const UNTOLD_NAMED: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Episode {
    pub(crate) project: String,
    pub(crate) scenes: Vec<Scene>,
    /// What the change under review did that had no room for a scene. It is still named, since a
    /// reader takes a missing line for something that did not happen.
    pub(crate) untold: Vec<Plot>,
}

impl Episode {
    pub(crate) fn untold_line(&self) -> Option<String> {
        let mut notes: Vec<String> = self.untold.iter().take(UNTOLD_NAMED).map(note).collect();
        let more = self.untold.len().saturating_sub(UNTOLD_NAMED);
        if more > 0 {
            notes.push(format!("and {more} more"));
        }
        (!notes.is_empty()).then(|| format!("Also tonight: {}.", notes.join(", ")))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Scene {
    pub(crate) text: String,
    /// The fact the line narrates; a quiet episode's only scene has none.
    pub(crate) plot: Option<Plot>,
}

impl Scene {
    pub(crate) fn is_tonight(&self) -> bool {
        self.plot
            .as_ref()
            .is_some_and(|plot| plot.kind().is_tonight())
    }
}

pub(crate) fn write(project: &str, mut plots: Vec<Plot>) -> Episode {
    let project = &tidy(project);
    plots.sort_by_cached_key(|plot| (Reverse(plot.juiciness()), seed(project, plot)));
    let mut cast: Vec<&Plot> = Vec::new();
    for plot in &plots {
        if has_room(&cast, plot.kind()) {
            cast.push(plot);
        }
    }
    let mut untold: Vec<Plot> = plots
        .iter()
        .filter(|plot| plot.kind().is_tonight() && !cast.contains(plot))
        .cloned()
        .collect();
    untold.sort_by(|a, b| (a.kind(), a.name()).cmp(&(b.kind(), b.name())));
    cast.sort_by_key(|plot| plot.kind().is_tonight());
    let mut scenes: Vec<Scene> = Vec::new();
    for plot in cast {
        let line = pick(&lines_for(plot), seed(project, plot), |line| {
            let text = fill(line, plot);
            scenes.iter().all(|scene| scene.text != text)
        });
        scenes.push(Scene {
            text: fill(line, plot),
            plot: Some(plot.clone()),
        });
    }
    if scenes.iter().all(Scene::is_tonight) {
        let quiet = lines_in(include_str!("lines/quiet.txt"));
        let scene = Scene {
            text: pick(&quiet, fnv1a(project), |_| true).to_owned(),
            plot: None,
        };
        scenes.insert(0, scene);
    }
    Episode {
        project: project.to_owned(),
        scenes,
        untold,
    }
}

fn note(plot: &Plot) -> String {
    let note = match plot {
        Plot::NewArrival { name, version } => format!("{name} {version} arrived"),
        Plot::WrittenOut { name, version } => format!("{name} {version} written out"),
        Plot::Recast { name, from, to, .. } => format!("{name} {from} recast as {to}"),
        other => other
            .name()
            .unwrap_or_else(|| other.kind().slug())
            .to_owned(),
    };
    tidy(&note)
}

fn has_room(cast: &[&Plot], kind: Kind) -> bool {
    let same_kind = cast.iter().filter(|plot| plot.kind() == kind).count();
    let same_act = cast
        .iter()
        .filter(|plot| plot.kind().is_tonight() == kind.is_tonight())
        .count();
    let act = if kind.is_tonight() {
        SCENES_TONIGHT
    } else {
        SCENES
    };
    let ambient = cast.iter().any(|plot| plot.kind().is_ambient());
    same_act < act && same_kind < SCENES_PER_KIND && !(kind.is_ambient() && ambient)
}

/// The seeded choice, moved along to the next line the episode has not used yet.
fn pick(lines: &[&'static str], seed: u64, fresh: impl Fn(&str) -> bool) -> &'static str {
    let start = usize::try_from(seed % (lines.len() as u64).max(1)).unwrap_or(0);
    let from_seed = || lines.iter().cycle().skip(start).take(lines.len()).copied();
    from_seed()
        .find(|line| fresh(line))
        .or_else(|| from_seed().next())
        .unwrap_or_default()
}

fn fill(line: &str, plot: &Plot) -> String {
    let text = plot
        .values()
        .into_iter()
        .fold(line.to_owned(), |text, (name, value)| {
            text.replace(&format!("{{{name}}}"), &tidy(&value))
        });
    match plot.aside() {
        Some(aside) => format!("{text} {aside}"),
        None => text,
    }
}

/// Names and messages are written by strangers: no line break or terminal escape code in one may
/// reach the output.
fn tidy(text: &str) -> String {
    text.split(|character: char| character.is_control() || character.is_whitespace())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A well-known package's own lines when it has any for this kind of scene, the general ones
/// otherwise.
fn lines_for(plot: &Plot) -> Vec<&'static str> {
    let own: Vec<_> = celebrity_lines()
        .filter(|(name, kind, _)| plot.name() == Some(name) && *kind == plot.kind().slug())
        .map(|(_, _, line)| line)
        .collect();
    if own.is_empty() {
        lines(plot.kind())
    } else {
        own
    }
}

/// Rows of `package | scene | line`.
fn celebrity_lines() -> impl Iterator<Item = (&'static str, &'static str, &'static str)> {
    lines_in(include_str!("lines/celebrities.txt"))
        .into_iter()
        .filter_map(|row| {
            let mut columns = row.splitn(3, " | ");
            Some((columns.next()?, columns.next()?, columns.next()?))
        })
}

fn lines(kind: Kind) -> Vec<&'static str> {
    lines_in(match kind {
        Kind::Entourage => include_str!("lines/entourage.txt"),
        Kind::AlmostReady => include_str!("lines/almost_ready.txt"),
        Kind::Ghost => include_str!("lines/ghost.txt"),
        Kind::Clones => include_str!("lines/clones.txt"),
        Kind::Feud => include_str!("lines/feud.txt"),
        Kind::TimeCapsule => include_str!("lines/time_capsule.txt"),
        Kind::MissingPerson => include_str!("lines/missing_person.txt"),
        Kind::CollectionPlate => include_str!("lines/collection_plate.txt"),
        Kind::NightVisitor => include_str!("lines/night_visitor.txt"),
        Kind::NewArrival => include_str!("lines/new_arrival.txt"),
        Kind::WrittenOut => include_str!("lines/written_out.txt"),
        Kind::Recast => include_str!("lines/recast.txt"),
    })
}

fn lines_in(file: &'static str) -> Vec<&'static str> {
    file.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

fn seed(project: &str, plot: &Plot) -> u64 {
    fnv1a(&format!("{project}\n{plot:?}"))
}

/// FNV-1a rather than std's hasher, whose output may change between Rust releases.
fn fnv1a(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::{Episode, SCENES, SCENES_TONIGHT, celebrity_lines, fnv1a, lines, lines_in, write};
    use crate::plots::{Kind, Plot};

    fn ghost(name: &str, direct: bool) -> Plot {
        Plot::Ghost {
            name: name.to_owned(),
            version: "1.0.0".to_owned(),
            message: "use something else".to_owned(),
            direct,
        }
    }

    fn almost_ready(name: &str) -> Plot {
        Plot::AlmostReady {
            name: name.to_owned(),
            version: "0.4.2".to_owned(),
        }
    }

    fn placeholders(line: &str) -> Vec<&str> {
        line.split('{')
            .skip(1)
            .filter_map(|rest| rest.split_once('}'))
            .map(|(name, _)| name)
            .collect()
    }

    #[test]
    fn a_project_without_plots_gets_a_quiet_episode() {
        let episode = write("calm", Vec::new());

        assert_eq!(episode.scenes.len(), 1);
        assert_eq!(episode.scenes[0].plot, None);
        assert!(
            lines_in(include_str!("lines/quiet.txt")).contains(&episode.scenes[0].text.as_str())
        );
    }

    #[test]
    fn one_plot_is_one_scene_with_its_facts_filled_in() {
        let episode = write("shop-web", vec![almost_ready("axios")]);

        assert_eq!(episode.scenes.len(), 1);
        assert_eq!(episode.scenes[0].plot, Some(almost_ready("axios")));
        assert!(episode.scenes[0].text.contains("axios"));
        assert!(episode.scenes[0].text.contains("0.4.2"));
        assert!(!episode.scenes[0].text.contains('{'));
    }

    #[test]
    fn line_breaks_and_escape_codes_in_a_name_or_a_message_never_reach_a_scene() {
        let plot = Plot::Ghost {
            name: "old\n## heading".to_owned(),
            version: "1.0.0".to_owned(),
            message: "\u{1b}[31mred\r\nalert\u{7}".to_owned(),
            direct: true,
        };

        let episode = write("shop\n\n# web\u{1b}", vec![plot]);

        assert_eq!(episode.project, "shop # web");
        assert!(
            episode.scenes[0].text.contains("old ## heading"),
            "{episode:?}"
        );
        assert!(
            episode.scenes[0].text.contains("[31mred alert"),
            "{episode:?}"
        );
        assert!(
            !episode.scenes[0].text.contains(char::is_control),
            "{episode:?}"
        );
    }

    #[test]
    fn the_same_facts_give_the_same_episode() {
        let plots = || {
            vec![
                ghost("request", true),
                almost_ready("axios"),
                Plot::CollectionPlate { packages: 46 },
                Plot::Entourage {
                    direct: 9,
                    installed: 486,
                },
            ]
        };

        assert_eq!(write("shop-web", plots()), write("shop-web", plots()));
    }

    #[test]
    fn the_order_plots_arrive_in_does_not_change_the_episode() {
        let plots = vec![
            ghost("request", true),
            almost_ready("axios"),
            almost_ready("esbuild"),
        ];
        let mut reversed = plots.clone();
        reversed.reverse();

        assert_eq!(write("shop-web", plots), write("shop-web", reversed));
    }

    #[test]
    fn an_episode_has_five_scenes_at_most() {
        let plots = vec![
            ghost("request", true),
            almost_ready("axios"),
            Plot::Entourage {
                direct: 9,
                installed: 486,
            },
            Plot::Clones {
                name: "ms".to_owned(),
                versions: 4,
            },
            Plot::CollectionPlate { packages: 46 },
            Plot::MissingPerson {
                name: "left-pad".to_owned(),
                since_year: 2018,
                years: 8,
            },
            Plot::Feud {
                name: "widget".to_owned(),
                peer: "react".to_owned(),
                range: "^18".to_owned(),
                installed: "16.14.0".to_owned(),
            },
        ];

        let episode = write("shop-web", plots);

        assert_eq!(episode.scenes.len(), SCENES);
        assert!(
            episode.scenes[0].text.contains("widget"),
            "the feud opens: {episode:?}"
        );
        assert!(
            !episode
                .scenes
                .iter()
                .any(|scene| scene.text.contains("npm fund"))
        );
    }

    fn arrival(name: &str) -> Plot {
        Plot::NewArrival {
            name: name.to_owned(),
            version: "1.0.0".to_owned(),
        }
    }

    #[test]
    fn what_the_change_did_is_told_last_and_on_top_of_the_five() {
        let mut plots = vec![arrival("left-pad")];
        plots.extend(["a", "b"].map(|name| ghost(name, true)));
        plots.extend(["c", "d"].map(almost_ready));
        plots.push(Plot::Entourage {
            direct: 4,
            installed: 486,
        });

        let episode = write("shop-web", plots);

        assert_eq!(episode.scenes.len(), SCENES + 1);
        let tonight: Vec<_> = episode
            .scenes
            .iter()
            .map(super::Scene::is_tonight)
            .collect();
        assert_eq!(tonight, [false, false, false, false, false, true]);
        assert_eq!(episode.scenes[5].plot, Some(arrival("left-pad")));
    }

    #[test]
    fn a_change_with_many_arrivals_gets_three_scenes_about_it_at_most() {
        let mut plots: Vec<_> = ["a", "b", "c"].map(arrival).to_vec();
        plots.extend(["d", "e"].map(|name| Plot::WrittenOut {
            name: name.to_owned(),
            version: "1.0.0".to_owned(),
        }));

        let episode = write("shop-web", plots);

        let tonight = episode.scenes.iter().filter(|scene| scene.is_tonight());
        assert_eq!(tonight.count(), SCENES_TONIGHT);
    }

    #[test]
    fn changes_without_a_scene_are_still_named_in_one_line() {
        let mut plots: Vec<_> = ["dayjs", "ms"].map(arrival).to_vec();
        plots.push(Plot::Recast {
            name: "eslint-plugin-unicorn".to_owned(),
            from: "61.0.2".to_owned(),
            to: "62.0.0".to_owned(),
            moved_in: 1,
            used_in: 1,
        });
        plots.push(Plot::WrittenOut {
            name: "@types/dompurify".to_owned(),
            version: "3.2.0".to_owned(),
        });

        let episode = write("shop-web", plots);

        assert_eq!(
            episode.untold_line().as_deref(),
            Some("Also tonight: @types/dompurify 3.2.0 written out.")
        );
        assert_eq!(write("shop-web", vec![arrival("a")]).untold_line(), None);
    }

    #[test]
    fn a_long_list_of_changes_without_a_scene_names_eight_and_counts_the_rest() {
        let names: Vec<String> = (0..20).map(|index| format!("p{index:02}")).collect();
        let plots: Vec<_> = names.iter().map(|name| arrival(name)).collect();

        let episode = write("shop-web", plots);

        assert_eq!(episode.untold.len(), 18);
        let line = episode.untold_line().expect("a line");
        assert_eq!(line.matches(" arrived").count(), 8, "{line}");
        assert!(line.ends_with(", and 10 more."), "{line}");
    }

    #[test]
    fn a_change_to_an_otherwise_quiet_project_still_opens_with_the_quiet_line() {
        let episode = write("calm", vec![arrival("left-pad")]);

        assert_eq!(episode.scenes.len(), 2);
        assert_eq!(episode.scenes[0].plot, None);
        assert!(episode.scenes[1].is_tonight());
    }

    #[test]
    fn two_ambient_scenes_never_share_an_episode() {
        for project in ["a", "b", "c", "d", "e", "f", "g", "h"] {
            let episode = write(
                project,
                vec![
                    Plot::CollectionPlate { packages: 46 },
                    Plot::NightVisitor {
                        packages: 3,
                        example: "esbuild".to_owned(),
                    },
                ],
            );

            assert_eq!(episode.scenes.len(), 1, "{episode:?}");
        }
    }

    #[test]
    fn one_kind_of_plot_gets_two_scenes_at_most_and_two_different_lines() {
        let ghosts = ["a", "b", "c", "d"].map(|name| ghost(name, false)).to_vec();

        let Episode { scenes, .. } = write("haunted", ghosts);

        assert_eq!(scenes.len(), 2);
        let shape = |scene: &str| {
            scene
                .split_whitespace()
                .skip(1)
                .take(3)
                .collect::<Vec<_>>()
                .join(" ")
        };
        assert_ne!(shape(&scenes[0].text), shape(&scenes[1].text));
    }

    #[test]
    fn a_direct_ghost_outranks_a_transitive_one() {
        let episode = write(
            "haunted",
            vec![ghost("deep", false), ghost("request", true)],
        );

        assert!(episode.scenes[0].text.contains("request"));
    }

    #[test]
    fn every_kind_has_lines_and_every_line_uses_only_what_its_kind_provides() {
        for kind in Kind::ALL {
            let lines = lines(kind);
            assert!(lines.len() >= 8, "{kind:?} has only {} lines", lines.len());
            let provided = kind.placeholders();
            let unknown: Vec<_> = lines
                .iter()
                .flat_map(|line| placeholders(line))
                .filter(|placeholder| !provided.contains(placeholder))
                .collect();
            assert_eq!(unknown, Vec::<&str>::new(), "{kind:?} cannot fill these");
        }
    }

    #[test]
    fn a_well_known_package_gets_its_own_line() {
        let episode = write("shop-web", vec![ghost("left-pad", true)]);

        assert!(episode.scenes[0].text.contains("2016"), "{episode:?}");
        assert!(!episode.scenes[0].text.contains('{'), "{episode:?}");
    }

    #[test]
    fn a_well_known_package_in_a_scene_without_its_own_line_gets_a_general_one() {
        let episode = write("shop-web", vec![almost_ready("left-pad")]);

        assert!(lines(Kind::AlmostReady).iter().any(|line| {
            line.replace("{name}", "left-pad")
                .replace("{version}", "0.4.2")
                == episode.scenes[0].text
        }));
    }

    #[test]
    fn every_row_for_a_well_known_package_names_a_kind_and_uses_only_its_placeholders() {
        let rows = lines_in(include_str!("lines/celebrities.txt"));
        assert_eq!(
            celebrity_lines().count(),
            rows.len(),
            "a row is missing a column"
        );

        for (name, slug, line) in celebrity_lines() {
            let kind = Kind::ALL.into_iter().find(|kind| kind.slug() == slug);
            let kind = kind.unwrap_or_else(|| panic!("{name}: no scene is called {slug}"));
            let unknown: Vec<_> = placeholders(line)
                .into_iter()
                .filter(|placeholder| !kind.placeholders().contains(placeholder))
                .collect();
            assert_eq!(unknown, Vec::<&str>::new(), "{name} | {slug}");
            assert!(
                kind.placeholders().contains(&"name"),
                "{slug} is not about a package"
            );
        }
    }

    #[test]
    fn no_line_is_written_twice() {
        for kind in Kind::ALL {
            let mut lines = lines(kind);
            let before = lines.len();
            lines.sort_unstable();
            lines.dedup();
            assert_eq!(lines.len(), before, "{kind:?}");
        }
    }

    #[test]
    fn quiet_lines_need_no_facts() {
        for line in lines_in(include_str!("lines/quiet.txt")) {
            assert!(placeholders(line).is_empty(), "{line}");
        }
    }

    #[test]
    fn the_seed_hash_is_pinned_so_episodes_survive_a_toolchain_upgrade() {
        assert_eq!(fnv1a(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a("a"), 0xaf63_dc4c_8601_ec8c);
    }
}
