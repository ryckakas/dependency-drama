//! Lays an episode out for a terminal or as Markdown for a pull request comment.

use crate::episode::Episode;

const HOMEPAGE: &str = env!("CARGO_PKG_REPOSITORY");

pub(crate) fn terminal(episode: &Episode) -> String {
    let mut text = format!("PREVIOUSLY ON {}…\n\n", episode.project);
    for scene in &episode.scenes {
        text.push_str("  ");
        text.push_str(scene);
        text.push('\n');
    }
    text
}

pub(crate) fn markdown(episode: &Episode) -> String {
    let mut text = format!(
        "### 📺 Previously on `{}`…\n\n",
        episode.project.replace('`', "")
    );
    for scene in &episode.scenes {
        text.push_str("- ");
        text.push_str(&escape(scene));
        text.push('\n');
    }
    text.push_str("\n<sub>Every line is a true fact from the lockfile, narrated by ");
    text.push_str("[dependency-drama](");
    text.push_str(HOMEPAGE);
    text.push_str(").</sub>\n");
    text
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
    use super::{markdown, terminal};
    use crate::episode::Episode;

    fn episode(scenes: &[&str]) -> Episode {
        Episode {
            project: "shop-web".to_owned(),
            scenes: scenes.iter().map(|scene| (*scene).to_owned()).collect(),
        }
    }

    #[test]
    fn the_terminal_shows_the_title_and_one_indented_line_per_scene() {
        let text = terminal(&episode(&["You invited 9 guests.", "Dinner was tense."]));

        assert_eq!(
            text,
            "PREVIOUSLY ON shop-web…\n\n  You invited 9 guests.\n  Dinner was tense.\n"
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
