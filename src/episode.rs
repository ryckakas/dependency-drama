//! Turns plot points into an episode: which scenes make the cut, and which line each one gets.

use std::cmp::Reverse;

use crate::plots::{Kind, Plot};

const SCENES: usize = 5;
/// Without this, a project with ten deprecated packages gets an episode about nothing else.
const SCENES_PER_KIND: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Episode {
    pub(crate) project: String,
    pub(crate) scenes: Vec<String>,
}

pub(crate) fn write(project: &str, mut plots: Vec<Plot>) -> Episode {
    let project = &tidy(project);
    plots.sort_by_cached_key(|plot| (Reverse(plot.juiciness()), seed(project, plot)));
    let mut cast: Vec<&Plot> = Vec::new();
    for plot in &plots {
        if cast.len() < SCENES && has_room(&cast, plot.kind()) {
            cast.push(plot);
        }
    }
    let mut scenes: Vec<String> = Vec::new();
    for plot in cast {
        let line = pick(&lines(plot.kind()), seed(project, plot), |line| {
            !scenes.contains(&fill(line, plot))
        });
        scenes.push(fill(line, plot));
    }
    if scenes.is_empty() {
        let quiet = lines_in(include_str!("lines/quiet.txt"));
        scenes.push(pick(&quiet, fnv1a(project), |_| true).to_owned());
    }
    Episode {
        project: project.to_owned(),
        scenes,
    }
}

fn has_room(cast: &[&Plot], kind: Kind) -> bool {
    let same_kind = cast.iter().filter(|plot| plot.kind() == kind).count();
    let ambient = cast.iter().any(|plot| plot.kind().is_ambient());
    same_kind < SCENES_PER_KIND && !(kind.is_ambient() && ambient)
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
    plot.values()
        .into_iter()
        .fold(line.to_owned(), |text, (name, value)| {
            text.replace(&format!("{{{name}}}"), &tidy(&value))
        })
}

/// Names and messages are written by strangers: no line break or terminal escape code in one may
/// reach the output.
fn tidy(text: &str) -> String {
    text.split(|character: char| character.is_control() || character.is_whitespace())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
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
    use super::{Episode, SCENES, fnv1a, lines, lines_in, write};
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
        assert!(lines_in(include_str!("lines/quiet.txt")).contains(&episode.scenes[0].as_str()));
    }

    #[test]
    fn one_plot_is_one_scene_with_its_facts_filled_in() {
        let episode = write("shop-web", vec![almost_ready("axios")]);

        assert_eq!(episode.scenes.len(), 1);
        assert!(episode.scenes[0].contains("axios"));
        assert!(episode.scenes[0].contains("0.4.2"));
        assert!(!episode.scenes[0].contains('{'));
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
        assert!(episode.scenes[0].contains("old ## heading"), "{episode:?}");
        assert!(episode.scenes[0].contains("[31mred alert"), "{episode:?}");
        assert!(!episode.scenes[0].contains(char::is_control), "{episode:?}");
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
            episode.scenes[0].contains("widget"),
            "the feud opens: {episode:?}"
        );
        assert!(
            !episode
                .scenes
                .iter()
                .any(|scene| scene.contains("npm fund"))
        );
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
        assert_ne!(shape(&scenes[0]), shape(&scenes[1]));
    }

    #[test]
    fn a_direct_ghost_outranks_a_transitive_one() {
        let episode = write(
            "haunted",
            vec![ghost("deep", false), ghost("request", true)],
        );

        assert!(episode.scenes[0].contains("request"));
    }

    #[test]
    fn every_kind_has_lines_and_every_line_uses_only_what_its_kind_provides() {
        for kind in Kind::ALL {
            let lines = lines(kind);
            assert!(lines.len() >= 4, "{kind:?} has only {} lines", lines.len());
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
