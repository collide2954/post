use std::collections::BTreeMap;
use std::fmt;

use pulldown_cmark::{CowStr, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd, html};

use crate::paths::{self, REPOSITORY};

const RECORD_LENGTH: usize = "D-000".len();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    File,
    Directory,
}

pub struct Context<'a> {
    pub source: &'a str,
    pub output: &'a str,
    pub documents: &'a BTreeMap<String, String>,
    pub records: &'a BTreeMap<String, String>,
    pub kind: &'a dyn Fn(&str) -> Option<Kind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub title: String,
    pub html: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    NoTitle,
    BrokenLink(String),
    Image(String),
}

pub fn render(markdown: &str, context: &Context<'_>) -> Result<Rendered, RenderError> {
    let mut events: Vec<Event<'_>> = Parser::new_ext(markdown, Options::ENABLE_TABLES).collect();
    let title = identify_headings(&mut events).ok_or(RenderError::NoTitle)?;
    let events = link(events, context)?;
    let mut html = String::new();
    html::push_html(&mut html, events.into_iter());
    Ok(Rendered { title, html })
}

#[must_use]
pub fn record_number(text: &str) -> Option<&str> {
    let number = text.get(..RECORD_LENGTH)?;
    let digits = number.strip_prefix("D-")?;
    let ends = text[RECORD_LENGTH..]
        .chars()
        .next()
        .is_none_or(|next| !next.is_ascii_alphanumeric());
    (digits.bytes().all(|byte| byte.is_ascii_digit()) && ends).then_some(number)
}

#[must_use]
pub fn record_identifier(number: &str) -> String {
    number.to_ascii_lowercase()
}

fn identify_headings(events: &mut [Event<'_>]) -> Option<String> {
    let mut title = None;
    let mut used = BTreeMap::new();
    for index in 0..events.len() {
        let Event::Start(Tag::Heading { level, .. }) = &events[index] else {
            continue;
        };
        let level = *level;
        let text = heading_text(&events[index + 1..]);
        let identifier = unique(identifier(&text), &mut used);
        if level == HeadingLevel::H1 && title.is_none() {
            title = Some(text);
        }
        if let Event::Start(Tag::Heading { id, .. }) = &mut events[index] {
            *id = Some(identifier.into());
        }
    }
    title
}

fn heading_text(events: &[Event<'_>]) -> String {
    let mut text = String::new();
    for event in events {
        match event {
            Event::End(TagEnd::Heading(_)) => break,
            Event::Text(part) | Event::Code(part) => text.push_str(part),
            _ => {}
        }
    }
    text
}

fn identifier(text: &str) -> String {
    if let Some(number) = record_number(text) {
        return record_identifier(number);
    }
    let mut identifier = String::new();
    for character in text.chars() {
        if character.is_ascii_alphanumeric() {
            identifier.push(character.to_ascii_lowercase());
        } else if (character.is_whitespace() || character == '-') && !identifier.ends_with('-') {
            identifier.push('-');
        }
    }
    let identifier = identifier.trim_matches('-');
    if identifier.is_empty() {
        "section".to_owned()
    } else {
        identifier.to_owned()
    }
}

fn unique(identifier: String, used: &mut BTreeMap<String, u32>) -> String {
    let count = used.entry(identifier.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        identifier
    } else {
        format!("{identifier}-{count}")
    }
}

fn link<'a>(events: Vec<Event<'a>>, context: &Context<'_>) -> Result<Vec<Event<'a>>, RenderError> {
    let mut output = Vec::with_capacity(events.len());
    let mut plain = 0_u32;
    for event in events {
        match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                plain += 1;
                output.push(Event::Start(Tag::Link {
                    link_type,
                    dest_url: destination(&dest_url, context)?.into(),
                    title,
                    id,
                }));
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                return Err(RenderError::Image(dest_url.into_string()));
            }
            Event::Start(tag @ (Tag::Heading { .. } | Tag::CodeBlock(_))) => {
                plain += 1;
                output.push(Event::Start(tag));
            }
            Event::End(end @ (TagEnd::Link | TagEnd::Heading(_) | TagEnd::CodeBlock)) => {
                plain -= 1;
                output.push(Event::End(end));
            }
            Event::Text(text) if plain == 0 => link_records(text, context, &mut output),
            other => output.push(other),
        }
    }
    Ok(output)
}

fn destination(link: &str, context: &Context<'_>) -> Result<String, RenderError> {
    let is_absolute = ["https://", "http://", "mailto:"]
        .iter()
        .any(|scheme| link.starts_with(scheme));
    if is_absolute || link.starts_with('#') {
        return Ok(link.to_owned());
    }

    let (path, fragment) = match link.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment)),
        None => (link, None),
    };
    let broken = || RenderError::BrokenLink(link.to_owned());
    let target = paths::resolve(context.source, path).ok_or_else(broken)?;
    let mut url = if let Some(page) = context.documents.get(&target) {
        paths::relative(context.output, page)
    } else {
        match (context.kind)(&target).ok_or_else(broken)? {
            Kind::File => format!("{REPOSITORY}/blob/main/{target}"),
            Kind::Directory => format!("{REPOSITORY}/tree/main/{target}"),
        }
    };
    if let Some(fragment) = fragment {
        url.push('#');
        url.push_str(fragment);
    }
    Ok(url)
}

fn link_records<'a>(text: CowStr<'a>, context: &Context<'_>, output: &mut Vec<Event<'a>>) {
    let mut done = 0;
    for (start, _) in text.match_indices("D-") {
        let follows_a_word = text[..start]
            .chars()
            .next_back()
            .is_some_and(|before| before.is_ascii_alphanumeric());
        let Some(number) = record_number(&text[start..]).filter(|_| !follows_a_word) else {
            continue;
        };
        let Some(page) = context.records.get(number) else {
            continue;
        };
        let url = format!(
            "{}#{}",
            paths::relative(context.output, page),
            record_identifier(number)
        );
        if done < start {
            output.push(Event::Text(text[done..start].to_owned().into()));
        }
        output.push(Event::Start(Tag::Link {
            link_type: LinkType::Inline,
            dest_url: url.into(),
            title: CowStr::Borrowed(""),
            id: CowStr::Borrowed(""),
        }));
        output.push(Event::Text(number.to_owned().into()));
        output.push(Event::End(TagEnd::Link));
        done = start + RECORD_LENGTH;
    }
    if done == 0 {
        output.push(Event::Text(text));
    } else if done < text.len() {
        output.push(Event::Text(text[done..].to_owned().into()));
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTitle => formatter.write_str("the document has no title"),
            Self::BrokenLink(link) => write!(formatter, "the link \"{link}\" goes to nothing"),
            Self::Image(image) => write!(
                formatter,
                "the image \"{image}\" is not possible: the website does not show images of documents"
            ),
        }
    }
}

impl std::error::Error for RenderError {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{Context, Kind, RenderError, Rendered, record_identifier, record_number, render};

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    fn kind(path: &str) -> Option<Kind> {
        match path {
            "docs/protocol/schemas" | "crates/post-proto" => Some(Kind::Directory),
            "docs/protocol/examples/errors.txt" | "LICENSE" => Some(Kind::File),
            _ => None,
        }
    }

    fn render_from(source: &str, output: &str, markdown: &str) -> Result<Rendered, RenderError> {
        let documents = map(&[
            ("docs/philosophy.md", "philosophy.html"),
            ("docs/decisions/README.md", "decisions/index.html"),
            (
                "docs/decisions/engineering.md",
                "decisions/engineering.html",
            ),
            ("docs/decisions/product.md", "decisions/product.html"),
            ("docs/protocol/README.md", "protocol/index.html"),
        ]);
        let records = map(&[
            ("D-003", "decisions/product.html"),
            ("D-100", "decisions/engineering.html"),
        ]);
        render(
            markdown,
            &Context {
                source,
                output,
                documents: &documents,
                records: &records,
                kind: &kind,
            },
        )
    }

    fn html(markdown: &str) -> String {
        render_from("docs/philosophy.md", "philosophy.html", markdown)
            .expect("a valid document")
            .html
    }

    #[test]
    fn gives_the_title_and_the_content() {
        let rendered = render_from(
            "docs/philosophy.md",
            "philosophy.html",
            "# Post design `philosophy`\n\nText with *weight*.\n",
        )
        .expect("a valid document");

        assert_eq!(rendered.title, "Post design philosophy");
        assert_eq!(
            rendered.html,
            "<h1 id=\"post-design-philosophy\">Post design <code>philosophy</code></h1>\n\
             <p>Text with <em>weight</em>.</p>\n"
        );
    }

    #[test]
    fn refuses_a_document_without_a_title() {
        let result = render_from(
            "docs/philosophy.md",
            "philosophy.html",
            "## Part\n\nText.\n",
        );

        assert_eq!(result, Err(RenderError::NoTitle));
    }

    #[test]
    fn gives_each_heading_an_identifier() {
        let content = html(
            "# Title\n\n## What Post is\n\n### 1. Simple first\n\n## What Post is\n\n\
             ## Caf\u{e9} -- and  more!\n\n## !!!\n\n## What Post is\n",
        );

        for identifier in [
            "<h1 id=\"title\">",
            "<h2 id=\"what-post-is\">",
            "<h3 id=\"1-simple-first\">",
            "<h2 id=\"what-post-is-2\">",
            "<h2 id=\"caf-and-more\">",
            "<h2 id=\"section\">",
            "<h2 id=\"what-post-is-3\">",
        ] {
            assert!(content.contains(identifier), "{identifier}\n{content}");
        }
    }

    #[test]
    fn gives_a_record_its_number_as_the_identifier() {
        let content = html("# Title\n\n### D-100 Roadmap\n\n### D-003 Scope of version 1.0\n");

        assert!(
            content.contains("<h3 id=\"d-100\">D-100 Roadmap</h3>"),
            "{content}"
        );
        assert!(content.contains("<h3 id=\"d-003\">D-003 Scope of version 1.0</h3>"));
    }

    #[test]
    fn makes_a_link_from_each_record_number() {
        let content = html("# Title\n\nSee D-100 and (D-003). D-100.\n");

        assert_eq!(
            content,
            "<h1 id=\"title\">Title</h1>\n\
             <p>See <a href=\"decisions/engineering.html#d-100\">D-100</a> and \
             (<a href=\"decisions/product.html#d-003\">D-003</a>). \
             <a href=\"decisions/engineering.html#d-100\">D-100</a>.</p>\n"
        );
    }

    #[test]
    fn gives_the_way_to_a_record_from_the_page_that_names_it() {
        let rendered = render_from(
            "docs/decisions/product.md",
            "decisions/product.html",
            "# Title\n\nSee D-100 and D-003.\n",
        )
        .expect("a valid document");

        assert!(
            rendered
                .html
                .contains("<a href=\"engineering.html#d-100\">D-100</a>")
        );
        assert!(
            rendered
                .html
                .contains("<a href=\"product.html#d-003\">D-003</a>")
        );
    }

    #[test]
    fn leaves_a_record_number_as_text_where_a_link_is_not_correct() {
        let content = html(
            "# Title D-100\n\nUse `D-100`, [the roadmap D-100](#x), D-999, D-1000, D-10, \
             XD-100, D-100x, and d-100.\n\n```\nD-100\n```\n",
        );

        assert_eq!(content.matches("<a ").count(), 1, "{content}");
        assert!(content.contains("<a href=\"#x\">the roadmap D-100</a>"));
    }

    #[test]
    fn changes_a_link_to_a_document_into_a_link_to_its_page() {
        let content = html(
            "# Title\n\n[records](decisions/README.md) [roadmap](decisions/engineering.md#d-100) \
             [protocol](protocol/README.md) [self](philosophy.md) [part](#what-post-is)\n",
        );

        for link in [
            "<a href=\"decisions/\">records</a>",
            "<a href=\"decisions/engineering.html#d-100\">roadmap</a>",
            "<a href=\"protocol/\">protocol</a>",
            "<a href=\"philosophy.html\">self</a>",
            "<a href=\"#what-post-is\">part</a>",
        ] {
            assert!(content.contains(link), "{link}\n{content}");
        }
    }

    #[test]
    fn gives_the_way_to_a_page_from_a_page_in_a_directory() {
        let rendered = render_from(
            "docs/decisions/engineering.md",
            "decisions/engineering.html",
            "# Title\n\n[guide](README.md) [ideas](../philosophy.md) [product](product.md)\n",
        )
        .expect("a valid document");

        for link in [
            "<a href=\"./\">guide</a>",
            "<a href=\"../philosophy.html\">ideas</a>",
            "<a href=\"product.html\">product</a>",
        ] {
            assert!(rendered.html.contains(link), "{link}\n{}", rendered.html);
        }
    }

    #[test]
    fn sends_a_link_to_a_different_file_to_the_repository() {
        let rendered = render_from(
            "docs/protocol/README.md",
            "protocol/index.html",
            "# Title\n\n[schemas](schemas) [errors](examples/errors.txt) \
             [crate](../../crates/post-proto) [license](../../LICENSE#top)\n",
        )
        .expect("a valid document");

        for link in [
            "<a href=\"https://github.com/collide2954/post/tree/main/docs/protocol/schemas\">",
            "<a href=\"https://github.com/collide2954/post/blob/main/docs/protocol/examples/errors.txt\">",
            "<a href=\"https://github.com/collide2954/post/tree/main/crates/post-proto\">",
            "<a href=\"https://github.com/collide2954/post/blob/main/LICENSE#top\">",
        ] {
            assert!(rendered.html.contains(link), "{link}\n{}", rendered.html);
        }
    }

    #[test]
    fn keeps_a_link_to_a_different_website() {
        let content = html(
            "# Title\n\n[rustup](https://rustup.rs) [plain](http://example.org/a.md) \
             [mail](mailto:person@example.org)\n",
        );

        assert!(content.contains("<a href=\"https://rustup.rs\">rustup</a>"));
        assert!(content.contains("<a href=\"http://example.org/a.md\">plain</a>"));
        assert!(content.contains("<a href=\"mailto:person@example.org\">mail</a>"));
    }

    #[test]
    fn refuses_a_link_that_goes_to_nothing() {
        for link in [
            "missing.md",
            "../../outside.md",
            "decisions/missing.md#d-100",
        ] {
            let markdown = format!("# Title\n\n[text]({link})\n");
            let result = render_from("docs/philosophy.md", "philosophy.html", &markdown);
            assert_eq!(result, Err(RenderError::BrokenLink(link.to_owned())));
        }
    }

    #[test]
    fn refuses_an_image() {
        let result = render_from(
            "docs/philosophy.md",
            "philosophy.html",
            "# Title\n\n![The window](images/chat.png)\n",
        );

        assert_eq!(
            result,
            Err(RenderError::Image("images/chat.png".to_owned()))
        );
    }

    #[test]
    fn shows_a_table() {
        let content = html("# Title\n\n| Version | Content |\n|---|---|\n| 0.1 | Slice D-100 |\n");

        assert!(content.contains("<table>"), "{content}");
        assert!(content.contains("<th>Version</th>"));
        assert!(
            content
                .contains("<td>Slice <a href=\"decisions/engineering.html#d-100\">D-100</a></td>")
        );
    }

    #[test]
    fn reads_a_record_number_at_the_start_of_a_text() {
        assert_eq!(record_number("D-100 Roadmap"), Some("D-100"));
        assert_eq!(record_number("D-100"), Some("D-100"));
        assert_eq!(record_number("D-100)."), Some("D-100"));
        for text in [
            "D-1000", "D-10", "D-10 x", "D-abc", "d-100", "X D-100", "D-100x", "",
        ] {
            assert_eq!(record_number(text), None, "{text}");
        }
        assert_eq!(record_identifier("D-100"), "d-100");
    }

    #[test]
    fn describes_each_error() {
        assert_eq!(
            RenderError::NoTitle.to_string(),
            "the document has no title"
        );
        assert_eq!(
            RenderError::BrokenLink("x.md".to_owned()).to_string(),
            "the link \"x.md\" goes to nothing"
        );
        assert_eq!(
            RenderError::Image("x.png".to_owned()).to_string(),
            "the image \"x.png\" is not possible: the website does not show images of documents"
        );
    }
}
