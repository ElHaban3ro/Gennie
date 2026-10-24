use pulldown_cmark::{Event, Parser, Tag, TagEnd};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Source {
    pub label: String,
    pub url: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Answer {
    pub text: String,
    pub sources: Vec<Source>,
}

fn valid_web_url(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://"))
        && !url.contains(char::is_whitespace)
        && url.len() <= 2048
}

fn domain(url: &str) -> String {
    url.split_once("://")
        .map(|(_, rest)| rest.split('/').next().unwrap_or(rest))
        .unwrap_or(url)
        .trim_start_matches("www.")
        .to_owned()
}

fn add_source(sources: &mut Vec<Source>, label: &str, url: &str) {
    if !valid_web_url(url) {
        return;
    }
    let label = label.trim();
    if let Some(existing) = sources.iter_mut().find(|source| source.url == url) {
        if !label.is_empty() {
            existing.label = label.to_owned();
        }
        return;
    }
    sources.push(Source {
        label: if label.is_empty() {
            domain(url)
        } else {
            label.to_owned()
        },
        url: url.to_owned(),
    });
}

fn extract_bare_urls(text: &str, sources: &mut Vec<Source>) -> String {
    let mut result = String::new();
    for token in text.split_whitespace() {
        let url = token.trim_end_matches([',', '.', ';', ')', ']']);
        if valid_web_url(url) {
            add_source(sources, "", url);
            if let Some(punctuation) = token.strip_prefix(url)
                && matches!(punctuation, "." | "," | ";")
            {
                result.push_str(punctuation);
            }
        } else {
            if !result.is_empty() {
                result.push(' ');
            }
            result.push_str(token);
        }
    }
    result
}

pub(crate) fn prepare(raw: &str) -> Answer {
    let mut body = String::new();
    let mut sources = Vec::new();
    let mut link: Option<(String, String, bool)> = None;
    for event in Parser::new(raw) {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => {
                let citation = body.trim_end().ends_with(['.', '!', '?', ':']);
                link = Some((dest_url.to_string(), String::new(), citation));
            }
            Event::End(TagEnd::Link) => {
                if let Some((url, label, citation)) = link.take() {
                    if valid_web_url(&url) {
                        add_source(&mut sources, &label, &url);
                        if !citation {
                            body.push_str(&label);
                        }
                    } else {
                        body.push_str(&label);
                    }
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, label, _)) = link.as_mut() {
                    label.push_str(&text);
                } else {
                    body.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => body.push(' '),
            Event::End(TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item) => body.push('\n'),
            _ => {}
        }
    }
    let text = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let mut text = extract_bare_urls(&text, &mut sources).trim().to_owned();
    if !sources.is_empty() {
        for heading in ["Fuentes:", "Sources:"] {
            if text.ends_with(heading) {
                text.truncate(text.len() - heading.len());
                text = text.trim_end().to_owned();
            }
        }
    }
    Answer { text, sources }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_inline_link_label_but_moves_trailing_citations() {
        let answer = prepare(
            "[Gennie](https://example.com/gennie) responde. [Fuente](https://example.com/source)",
        );
        assert_eq!(answer.text, "Gennie responde.");
        assert_eq!(answer.sources.len(), 2);
    }
    #[test]
    fn extracts_bare_urls_and_deduplicates() {
        let answer =
            prepare("Verificado en https://example.com/a. [Ejemplo](https://example.com/a)");
        assert_eq!(answer.sources.len(), 1);
        assert!(!answer.text.contains("https://"));
        assert_eq!(answer.sources[0].label, "Ejemplo");
    }
    #[test]
    fn removes_sources_heading_from_spoken_answer() {
        let answer = prepare("Respuesta breve.\n\nFuentes:\n[Ejemplo](https://example.com/a)");
        assert_eq!(answer.text, "Respuesta breve.");
        assert_eq!(answer.sources.len(), 1);
    }
    #[test]
    fn keeps_link_label_readable_for_saved_chat() {
        let answer = prepare(
            "Descárgalo desde [bluestacks.com](https://www.bluestacks.com/) y revisa las opciones.",
        );
        assert_eq!(
            answer.text,
            "Descárgalo desde bluestacks.com y revisa las opciones."
        );
        assert_eq!(answer.sources[0].label, "bluestacks.com");
        assert_eq!(answer.sources[0].url, "https://www.bluestacks.com/");
    }
}
