use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

const BASE: &str = "/post/";
const HOME_PAGES: [&str; 2] = ["index.html", "es/index.html"];

struct Site {
    directory: PathBuf,
    pages: BTreeMap<String, String>,
}

impl Site {
    fn build() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let name = format!(
            "post-site-repository-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let directory = std::env::temp_dir().join(name);
        post_site::build(&repository(), &directory).expect("the website of the repository");

        let mut pages = BTreeMap::new();
        for path in files(&directory, "") {
            let is_page = Path::new(&path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("html"));
            if is_page {
                let text = fs::read_to_string(directory.join(&path)).expect("a page");
                pages.insert(path, text);
            }
        }
        Self { directory, pages }
    }

    fn has(&self, path: &str) -> bool {
        self.directory.join(path).is_file()
    }

    fn page(&self, path: &str) -> &str {
        self.pages
            .get(path)
            .unwrap_or_else(|| panic!("the page {path} is missing"))
    }
}

impl Drop for Site {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn files(directory: &Path, prefix: &str) -> Vec<String> {
    let mut names = Vec::new();
    for entry in fs::read_dir(directory).expect("a directory") {
        let entry = entry.expect("an entry");
        let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
        if entry.file_type().expect("a file type").is_dir() {
            names.extend(files(&entry.path(), &format!("{name}/")));
        } else {
            names.push(name);
        }
    }
    names
}

fn attributes<'a>(html: &'a str, name: &str) -> Vec<&'a str> {
    let start = format!(" {name}=\"");
    html.split(start.as_str())
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .collect()
}

fn tags<'a>(html: &'a str, name: &str) -> Vec<&'a str> {
    let start = format!("<{name} ");
    html.split(start.as_str())
        .skip(1)
        .filter_map(|rest| rest.split('>').next())
        .collect()
}

fn is_remote(link: &str) -> bool {
    ["https://", "http://", "mailto:", "//"]
        .iter()
        .any(|start| link.starts_with(start))
}

fn target(page: &str, link: &str) -> Option<(String, Option<String>)> {
    let (path, fragment) = match link.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment.to_owned())),
        None => (link, None),
    };
    let mut parts: Vec<&str> = match path.strip_prefix(BASE) {
        Some(_) => Vec::new(),
        None => page.split('/').collect(),
    };
    parts.pop();
    let path = path.strip_prefix(BASE).unwrap_or(path);
    if path.is_empty() && fragment.is_some() {
        return Some((page.to_owned(), fragment));
    }
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(name),
        }
    }
    if path.is_empty() || path.ends_with('/') || path.ends_with('.') {
        parts.push("index.html");
    }
    Some((parts.join("/"), fragment))
}

#[test]
fn makes_each_page_and_each_file_of_the_website() {
    let site = Site::build();

    let mut expected = vec![
        "index.html".to_owned(),
        "es/index.html".to_owned(),
        "404.html".to_owned(),
        "style.css".to_owned(),
        "icon.svg".to_owned(),
        "fonts/Inter-Regular.woff2".to_owned(),
        "fonts/Inter-SemiBold.woff2".to_owned(),
        "fonts/JetBrainsMono-Regular.woff2".to_owned(),
        "fonts/Inter-LICENSE.txt".to_owned(),
        "fonts/JetBrainsMono-LICENSE.txt".to_owned(),
        "images/chat-light.png".to_owned(),
        "images/chat-dark.png".to_owned(),
        "philosophy.html".to_owned(),
        "architecture.html".to_owned(),
        "threat-model.html".to_owned(),
        "changelog.html".to_owned(),
        "protocol/index.html".to_owned(),
        "decisions/index.html".to_owned(),
    ];
    for name in files(&repository().join("docs").join("decisions"), "") {
        if let Some(area) = name.strip_suffix(".md").filter(|area| *area != "README") {
            expected.push(format!("decisions/{area}.html"));
        }
    }

    for path in expected {
        assert!(site.has(&path), "{path} is missing");
    }
    assert!(!site.has("document.html"));
    assert!(!site.has("mark.svg"));
}

#[test]
fn leaves_no_placeholder_in_a_page() {
    let site = Site::build();

    for (path, html) in &site.pages {
        for placeholder in [
            "{{root}}",
            "{{mark}}",
            "{{title}}",
            "{{content}}",
            "{{source}}",
        ] {
            assert!(!html.contains(placeholder), "{path} contains {placeholder}");
        }
        assert!(html.starts_with("<!doctype html>"), "{path}");
        assert!(html.contains("<title>"), "{path}");
    }
}

#[test]
fn each_link_goes_to_something_that_exists() {
    let site = Site::build();
    let mut examined = 0;

    for (page, html) in &site.pages {
        let links = ["href", "src", "srcset"]
            .into_iter()
            .flat_map(|name| attributes(html, name));
        for link in links {
            if is_remote(link) {
                continue;
            }
            examined += 1;
            let (path, fragment) = target(page, link)
                .unwrap_or_else(|| panic!("{page}: {link} goes out of the website"));
            assert!(
                site.has(&path),
                "{page}: {link} goes to {path}, which is missing"
            );
            if let Some(fragment) = fragment {
                let anchor = format!(" id=\"{fragment}\"");
                assert!(
                    site.page(&path).contains(&anchor),
                    "{page}: {link} goes to an anchor that {path} does not have"
                );
            }
        }
    }
    assert!(examined > 500, "only {examined} links");
}

#[test]
fn loads_nothing_from_a_different_server() {
    let site = Site::build();

    for (page, html) in &site.pages {
        assert!(!html.contains("<script"), "{page} has a script");
        assert!(!html.contains("<iframe"), "{page} has a frame");
        for source in attributes(html, "src")
            .into_iter()
            .chain(attributes(html, "srcset"))
        {
            assert!(!is_remote(source), "{page} loads {source}");
        }
        for tag in tags(html, "link") {
            let points_only =
                tag.contains("rel=\"canonical\"") || tag.contains("rel=\"alternate\"");
            let remote = attributes(&format!(" {tag}"), "href")
                .into_iter()
                .any(is_remote);
            assert!(points_only || !remote, "{page} loads <link {tag}>");
        }
    }

    let styles = fs::read_to_string(site.directory.join("style.css")).expect("the style sheet");
    assert!(!styles.contains("@import"));
    for address in styles.split("url(").skip(1) {
        let address = address.trim_start_matches(['"', '\'']);
        assert!(!is_remote(address), "the style sheet loads {address}");
    }
}

#[test]
fn gives_each_decision_record_an_anchor_and_a_link() {
    let site = Site::build();
    let decisions = repository().join("docs").join("decisions");
    let mut records = 0;
    for name in files(&decisions, "") {
        let text = fs::read_to_string(decisions.join(&name)).expect("a document");
        records += text
            .lines()
            .filter(|line| line.starts_with("### D-"))
            .count();
    }

    let index = site.page("decisions/index.html");
    let linked: BTreeSet<&str> = attributes(index, "href")
        .into_iter()
        .filter_map(|link| link.split_once(".html#d-"))
        .map(|(_, number)| number)
        .collect();

    assert!(records >= 175, "only {records} records");
    assert_eq!(linked.len(), records);
}

#[test]
fn has_a_home_page_in_each_language() {
    let site = Site::build();
    let english = site.page("index.html");
    let spanish = site.page("es/index.html");

    assert!(english.contains("<html lang=\"en\">"));
    assert!(spanish.contains("<html lang=\"es-419\">"));
    assert!(attributes(english, "href").contains(&"./es/"));
    assert!(attributes(spanish, "href").contains(&"../"));
}

#[test]
fn keeps_the_two_home_pages_equal_in_structure() {
    let site = Site::build();
    let shape = |page: &str| -> Vec<usize> {
        let html = site.page(page);
        [
            "<section", "<h1", "<h2", "<h3", "<li>", "<p", "<a ", "<img ", " id=\"",
        ]
        .iter()
        .map(|part| html.matches(part).count())
        .collect()
    };

    assert_eq!(shape(HOME_PAGES[0]), shape(HOME_PAGES[1]));

    let local = |page: &str| -> Vec<String> {
        attributes(site.page(page), "href")
            .into_iter()
            .filter(|link| !is_remote(link) && !link.starts_with('#'))
            .filter_map(|link| target(page, link))
            .map(|(path, fragment)| format!("{path}#{}", fragment.unwrap_or_default()))
            .filter(|link| !HOME_PAGES.iter().any(|home| link.starts_with(home)))
            .collect()
    };
    assert_eq!(local(HOME_PAGES[0]), local(HOME_PAGES[1]));
}
