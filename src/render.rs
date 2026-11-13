//! Lays an episode out for a terminal, as Markdown for a pull request comment, or as JSON for a
//! tool that writes its own comment.

use serde::Serialize;

use crate::episode::{Episode, Scene};
use crate::plots::Plot;

const HOMEPAGE: &str = env!("CARGO_PKG_REPOSITORY");

pub(crate) fn terminal(episode: &Episode) -> String {
    let (previously, tonight) = acts(episode);
    let mut text = format!("PREVIOUSLY ON {}…\n\n", episode.project);
    for scene in previously {
        push_line(&mut text, "  ", &scene.text);
    }
    if !tonight.is_empty() {
        text.push_str("\nTONIGHT…\n\n");
    }
    for scene in tonight {
        push_line(&mut text, "  ", &scene.text);
    }
    if let Some(line) = episode.untold_line() {
        push_line(&mut text, "  ", &line);
    }
    text
}

fn push_line(text: &mut String, lead: &str, line: &str) {
    text.push_str(lead);
    text.push_str(line);
    text.push('\n');
}

/// What was already true, then what the change under review did.
fn acts(episode: &Episode) -> (Vec<&Scene>, Vec<&Scene>) {
    episode.scenes.iter().partition(|scene| !scene.is_tonight())
}

pub(crate) fn markdown(episode: &Episode) -> String {
    let mut text = format!(
        "### 📺 Previously on `{}`…\n\n",
        episode.project.replace('`', "")
    );
    let (previously, tonight) = acts(episode);
    for scene in previously {
        push_line(&mut text, "- ", &escape(&scene.text));
    }
    if !tonight.is_empty() {
        text.push_str("\n**Tonight…**\n\n");
    }
    for scene in tonight {
        push_line(&mut text, "- ", &escape(&scene.text));
    }
    if let Some(line) = episode.untold_line() {
        push_line(&mut text, "- ", &escape(&line));
    }
    text.push_str("\n<sub>Every line is a true fact from the lockfile, narrated by ");
    text.push_str("[dependency-drama](");
    text.push_str(HOMEPAGE);
    text.push_str(").</sub>\n");
    text
}

/// Raised when a field is removed or changes meaning; adding a field or a kind does not raise it.
const SCHEMA: u32 = 1;

#[derive(Serialize)]
struct Document<'episode> {
    schema: u32,
    project: &'episode str,
    scenes: Vec<SceneDocument<'episode>>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    untold: &'episode [Plot],
}

#[derive(Serialize)]
struct SceneDocument<'episode> {
    #[serde(flatten)]
    subject: Subject<'episode>,
    text: &'episode str,
    markdown: String,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Subject<'episode> {
    Plot(&'episode Plot),
    Quiet { kind: &'static str },
}

pub(crate) fn json(episode: &Episode) -> String {
    let document = Document {
        schema: SCHEMA,
        project: &episode.project,
        scenes: episode.scenes.iter().map(scene_document).collect(),
        untold: &episode.untold,
    };
    let mut text = serde_json::to_string_pretty(&document).unwrap_or_default();
    text.push('\n');
    text
}

fn scene_document(scene: &Scene) -> SceneDocument<'_> {
    SceneDocument {
        subject: scene
            .plot
            .as_ref()
            .map_or(Subject::Quiet { kind: "quiet" }, Subject::Plot),
        text: &scene.text,
        markdown: escape(&scene.text),
    }
}

/// Deprecation messages are written by strangers, so nothing in a scene may act as markup.
fn escape(scene: &str) -> String {
    let mut escaped = String::with_capacity(scene.len());
    for character in scene.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '@' => escaped.push_str("@\u{200b}"),
            '\\' | '`' | '*' | '_' | '[' | ']' | '#' | '|' | '~' => {
                escaped.push('\\');
                escaped.push(character);
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{json, markdown, terminal};
    use crate::episode::{Episode, Scene};
    use crate::plots::Plot;

    fn episode(scenes: &[&str]) -> Episode {
        Episode {
            project: "shop-web".to_owned(),
            scenes: scenes.iter().map(|text| scene(text, None)).collect(),
            untold: Vec::new(),
        }
    }

    fn scene(text: &str, plot: Option<Plot>) -> Scene {
        Scene {
            text: text.to_owned(),
            plot,
        }
    }

    fn parsed(episode: &Episode) -> Value {
        serde_json::from_str(&json(episode)).expect("valid JSON")
    }

    #[test]
    fn json_gives_each_scene_its_kind_its_facts_and_its_line_in_both_forms() {
        let feud = Plot::Feud {
            name: "widget".to_owned(),
            peer: "react".to_owned(),
            range: "^18".to_owned(),
            installed: "16.14.0".to_owned(),
        };
        let episode = Episode {
            project: "shop-web".to_owned(),
            scenes: vec![scene("widget wanted react ^18_x.", Some(feud))],
            untold: Vec::new(),
        };

        assert_eq!(
            parsed(&episode),
            json!({
                "schema": 1,
                "project": "shop-web",
                "scenes": [{
                    "kind": "feud",
                    "facts": {
                        "name": "widget",
                        "peer": "react",
                        "range": "^18",
                        "installed": "16.14.0"
                    },
                    "text": "widget wanted react ^18_x.",
                    "markdown": "widget wanted react ^18\\_x."
                }]
            })
        );
    }

    #[test]
    fn json_names_kinds_and_facts_in_snake_case() {
        let capsule = Plot::TimeCapsule {
            name: "webpack".to_owned(),
            version: "4.47.0".to_owned(),
            latest: "5.99.0".to_owned(),
            majors_behind: 1,
        };
        let episode = Episode {
            project: "shop-web".to_owned(),
            scenes: vec![scene("webpack is behind.", Some(capsule))],
            untold: Vec::new(),
        };

        let scenes = parsed(&episode)["scenes"].clone();

        assert_eq!(scenes[0]["kind"], "time_capsule");
        assert_eq!(scenes[0]["facts"]["majors_behind"], 1);
    }

    #[test]
    fn json_marks_the_scene_of_a_quiet_episode_and_gives_it_no_facts() {
        let scenes = parsed(&episode(&["Nothing happened."]))["scenes"].clone();

        assert_eq!(
            scenes,
            json!([{"kind": "quiet", "text": "Nothing happened.", "markdown": "Nothing happened."}])
        );
    }

    #[test]
    fn json_without_scenes_is_still_a_document() {
        assert_eq!(
            parsed(&episode(&[])),
            json!({"schema": 1, "project": "shop-web", "scenes": []})
        );
    }

    #[test]
    fn json_ends_with_a_line_break_and_escapes_what_strangers_wrote() {
        let text = json(&episode(&["a \"quoted\" note\u{7}"]));

        assert!(text.ends_with("}\n"));
        assert!(text.contains(r#"a \"quoted\" note\u0007"#), "{text}");
    }

    #[test]
    fn the_terminal_shows_the_title_and_one_indented_line_per_scene() {
        let text = terminal(&episode(&["You invited 9 guests.", "Dinner was tense."]));

        assert_eq!(
            text,
            "PREVIOUSLY ON shop-web…\n\n  You invited 9 guests.\n  Dinner was tense.\n"
        );
    }

    fn with_an_arrival() -> Episode {
        let arrival = Plot::NewArrival {
            name: "left-pad".to_owned(),
            version: "1.3.0".to_owned(),
        };
        Episode {
            project: "shop-web".to_owned(),
            scenes: vec![
                scene("Dinner was tense.", None),
                scene("A stranger arrives: left-pad 1.3.0.", Some(arrival)),
            ],
            untold: Vec::new(),
        }
    }

    fn with_an_untold_departure() -> Episode {
        Episode {
            untold: vec![Plot::WrittenOut {
                name: "@types/dompurify".to_owned(),
                version: "3.2.0".to_owned(),
            }],
            ..with_an_arrival()
        }
    }

    #[test]
    fn every_layout_names_the_changes_that_got_no_scene() {
        let episode = with_an_untold_departure();

        assert!(terminal(&episode).ends_with(
            "  A stranger arrives: left-pad 1.3.0.\n  Also tonight: @types/dompurify 3.2.0 written out.\n"
        ));
        assert!(markdown(&episode).contains(
            "- A stranger arrives: left-pad 1.3.0.\n- Also tonight: @\u{200b}types/dompurify 3.2.0 written out.\n\n<sub>"
        ));
        assert_eq!(
            parsed(&episode)["untold"],
            json!([{"kind": "written_out", "facts": {"name": "@types/dompurify", "version": "3.2.0"}}])
        );
        assert_eq!(parsed(&with_an_arrival()).get("untold"), None);
    }

    #[test]
    fn the_terminal_tells_what_the_change_did_under_its_own_heading() {
        assert_eq!(
            terminal(&with_an_arrival()),
            "PREVIOUSLY ON shop-web…\n\n  Dinner was tense.\n\nTONIGHT…\n\n  A stranger arrives: left-pad 1.3.0.\n"
        );
    }

    #[test]
    fn markdown_tells_what_the_change_did_under_its_own_heading() {
        let text = markdown(&with_an_arrival());

        assert!(
            text.contains("- Dinner was tense.\n\n**Tonight…**\n\n- A stranger arrives: left-pad 1.3.0.\n\n<sub>"),
            "{text}"
        );
    }

    #[test]
    fn markdown_is_a_heading_a_list_and_a_credit() {
        let text = markdown(&episode(&["Dinner was tense."]));

        assert!(
            text.starts_with("### 📺 Previously on `shop-web`…\n\n- Dinner was tense.\n\n<sub>")
        );
        assert!(text.contains("https://github.com/ryckakas/dependency-drama"));
    }

    #[test]
    fn a_scene_cannot_inject_markup_or_mention_anyone() {
        let text = markdown(&episode(&[
            "<img src=x> see [docs](http://x) cc @someone `rm` #1 &commat;x",
        ]));

        assert!(text.contains(
            "- &lt;img src=x&gt; see \\[docs\\](http://x) cc @\u{200b}someone \\`rm\\` \\#1 &amp;commat;x\n"
        ));
    }
}
