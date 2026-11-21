use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::markdown::{Context, Kind, RenderError, record_number, render};
use crate::paths::{self, REPOSITORY};

const SOURCES: &str = "site";
const TEMPLATE: &str = "document.html";
const MARK: &str = "mark.svg";
const DECISIONS: &str = "docs/decisions";
const GUIDE: &str = "README.md";
const DOCUMENTS: [(&str, &str); 7] = [
    ("docs/philosophy.md", "philosophy.html"),
    ("docs/decisions/README.md", "decisions/index.html"),
    ("docs/protocol/README.md", "protocol/index.html"),
    ("docs/architecture.md", "architecture.html"),
    ("docs/threat-model.md", "threat-model.html"),
    ("docs/verification.md", "verification.html"),
    ("CHANGELOG.md", "changelog.html"),
];
const COPIES: [(&str, &str, &str); 2] = [
    ("docs/images", ".png", "images"),
    ("crates/post-app/fonts", "-LICENSE.txt", "fonts"),
];

#[derive(Debug)]
pub enum BuildError {
    File { path: PathBuf, error: io::Error },
    Document { source: String, error: RenderError },
    Placeholder { page: String, name: String },
}

pub fn build(root: &Path, out: &Path) -> Result<(), BuildError> {
    let sources = root.join(SOURCES);
    let mark = read(&sources.join(MARK))?;
    let template = read(&sources.join(TEMPLATE))?;

    for page in files(&sources, "")? {
        let from = sources.join(&page);
        let to = out.join(&page);
        if page == TEMPLATE || page == MARK {
            continue;
        }
        if has_extension(&page, "html") {
            let values = [("mark", mark.as_str()), ("root", &paths::root(&page))];
            write(&to, &fill(&read(&from)?, &values, &page)?)?;
        } else {
            copy(&from, &to)?;
        }
    }

    for (directory, end, target) in COPIES {
        let from = root.join(directory);
        for name in files(&from, "")? {
            if name.ends_with(end) {
                copy(&from.join(&name), &out.join(target).join(&name))?;
            }
        }
    }

    let documents = documents(root)?;
    let records = records(root, &documents)?;
    let kind = |path: &str| {
        let metadata = fs::metadata(root.join(path)).ok()?;
        Some(if metadata.is_dir() {
            Kind::Directory
        } else {
            Kind::File
        })
    };
    for (source, output) in &documents {
        let context = Context {
            source,
            output,
            documents: &documents,
            records: &records,
            kind: &kind,
        };
        let rendered =
            render(&read(&root.join(source))?, &context).map_err(|error| BuildError::Document {
                source: source.clone(),
                error,
            })?;
        let values = [
            ("mark", mark.as_str()),
            ("root", &paths::root(output)),
            ("title", &escape(&rendered.title)),
            ("content", &rendered.html),
            ("source", &format!("{REPOSITORY}/blob/main/{source}")),
        ];
        write(&out.join(output), &fill(&template, &values, TEMPLATE)?)?;
    }
    Ok(())
}

pub(crate) fn fill(
    template: &str,
    values: &[(&str, &str)],
    page: &str,
) -> Result<String, BuildError> {
    let mut filled = String::with_capacity(template.len());
    let mut rest = template;
    while let Some((before, after)) = rest.split_once("{{") {
        let unknown = |name: &str| BuildError::Placeholder {
            page: page.to_owned(),
            name: name.to_owned(),
        };
        let (name, after) = after.split_once("}}").ok_or_else(|| unknown(after))?;
        let (_, value) = values
            .iter()
            .find(|(known, _)| *known == name)
            .ok_or_else(|| unknown(name))?;
        filled.push_str(before);
        filled.push_str(value);
        rest = after;
    }
    filled.push_str(rest);
    Ok(filled)
}

pub(crate) fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            other => escaped.push(other),
        }
    }
    escaped
}

fn documents(root: &Path) -> Result<BTreeMap<String, String>, BuildError> {
    let mut documents: BTreeMap<String, String> = DOCUMENTS
        .iter()
        .map(|(source, output)| ((*source).to_owned(), (*output).to_owned()))
        .collect();
    for name in files(&root.join(DECISIONS), "")? {
        if let Some(area) = name.strip_suffix(".md").filter(|_| name != GUIDE) {
            documents.insert(
                format!("{DECISIONS}/{name}"),
                format!("decisions/{area}.html"),
            );
        }
    }
    Ok(documents)
}

fn records(
    root: &Path,
    documents: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, BuildError> {
    let mut records = BTreeMap::new();
    for (source, output) in documents {
        if !source.starts_with(DECISIONS) {
            continue;
        }
        for line in read(&root.join(source))?.lines() {
            if let Some(number) = line.strip_prefix("### ").and_then(record_number) {
                records.insert(number.to_owned(), output.clone());
            }
        }
    }
    Ok(records)
}

fn files(directory: &Path, prefix: &str) -> Result<Vec<String>, BuildError> {
    let failed = |error| BuildError::File {
        path: directory.to_owned(),
        error,
    };
    let mut names = Vec::new();
    for entry in fs::read_dir(directory).map_err(failed)? {
        let entry = entry.map_err(failed)?;
        let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
        if entry.file_type().map_err(failed)?.is_dir() {
            names.extend(files(&entry.path(), &format!("{name}/"))?);
        } else {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

fn has_extension(name: &str, extension: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|found| found.eq_ignore_ascii_case(extension))
}

fn read(path: &Path) -> Result<String, BuildError> {
    fs::read_to_string(path).map_err(|error| BuildError::File {
        path: path.to_owned(),
        error,
    })
}

fn write(path: &Path, text: &str) -> Result<(), BuildError> {
    make_parent(path)?;
    fs::write(path, text).map_err(|error| BuildError::File {
        path: path.to_owned(),
        error,
    })
}

fn copy(from: &Path, to: &Path) -> Result<(), BuildError> {
    make_parent(to)?;
    fs::copy(from, to)
        .map(drop)
        .map_err(|error| BuildError::File {
            path: from.to_owned(),
            error,
        })
}

fn make_parent(path: &Path) -> Result<(), BuildError> {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    fs::create_dir_all(parent).map_err(|error| BuildError::File {
        path: parent.to_owned(),
        error,
    })
}

impl fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File { path, error } => write!(formatter, "{}: {error}", path.display()),
            Self::Document { source, error } => write!(formatter, "{source}: {error}"),
            Self::Placeholder { page, name } => {
                write!(formatter, "{page}: the placeholder \"{name}\" is not known")
            }
        }
    }
}

impl std::error::Error for BuildError {}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::{BuildError, build, escape, fill};
    use crate::markdown::RenderError;

    struct Directory(PathBuf);

    impl Directory {
        fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let name = format!(
                "post-site-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let path = std::env::temp_dir().join(name);
            fs::create_dir_all(&path).expect("a temporary directory");
            Self(path)
        }

        fn put(&self, path: &str, text: &str) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
            fs::write(path, text).expect("a file");
        }

        fn get(&self, path: &str) -> String {
            fs::read_to_string(self.0.join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
        }
    }

    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn repository() -> Directory {
        let root = Directory::new();
        root.put("site/mark.svg", "<svg>mark</svg>");
        root.put(
            "site/document.html",
            "<title>{{title}}</title><a href=\"{{root}}style.css\">{{mark}}</a>\
             <main>{{content}}</main><a href=\"{{source}}\">source</a>",
        );
        root.put("site/index.html", "<a href=\"{{root}}es/\">{{mark}}</a>");
        root.put("site/es/index.html", "<a href=\"{{root}}\">{{mark}}</a>");
        root.put("site/style.css", "body { margin: 0 }");
        root.put("site/fonts/Inter.woff2", "font");
        root.put("docs/images/chat-light.png", "image");
        root.put("docs/images/notes.txt", "not an image");
        root.put("crates/post-app/fonts/Inter-LICENSE.txt", "license");
        root.put("crates/post-app/fonts/Inter-Regular.ttf", "font");
        root.put("docs/philosophy.md", "# Ideas & \"rules\"\n\nSee D-002.\n");
        root.put(
            "docs/decisions/README.md",
            "# Decision records\n\n- D-001 First\n- D-002 Second\n",
        );
        root.put(
            "docs/decisions/product.md",
            "# Product\n\nRead the [guide](README.md).\n\n### D-001 First\n\nText.\n",
        );
        root.put(
            "docs/decisions/server.md",
            "# Server\n\n### D-002 Second\n\nSee D-001.\n",
        );
        root.put("docs/decisions/notes.txt", "not a document");
        root.put(
            "docs/protocol/README.md",
            "# Protocol\n\n[schemas](schemas)\n",
        );
        root.put("docs/protocol/schemas/a.json", "{}");
        root.put("docs/architecture.md", "# Architecture\n");
        root.put("docs/threat-model.md", "# Threat model\n");
        root.put("docs/verification.md", "# Verification\n");
        root.put("CHANGELOG.md", "# Changelog\n\nRecord D-001.\n");
        root
    }

    fn built(root: &Directory) -> Directory {
        let out = Directory::new();
        build(&root.0, &out.0).expect("a website");
        out
    }

    #[test]
    fn copies_the_files_of_the_website() {
        let out = built(&repository());

        assert_eq!(out.get("style.css"), "body { margin: 0 }");
        assert_eq!(out.get("fonts/Inter.woff2"), "font");
        assert_eq!(out.get("images/chat-light.png"), "image");
        assert_eq!(out.get("fonts/Inter-LICENSE.txt"), "license");
        for absent in [
            "document.html",
            "mark.svg",
            "images/notes.txt",
            "fonts/Inter-Regular.ttf",
            "decisions/notes.html",
        ] {
            assert!(!out.0.join(absent).exists(), "{absent}");
        }
    }

    #[test]
    fn fills_the_pages_of_the_website() {
        let out = built(&repository());

        assert_eq!(
            out.get("index.html"),
            "<a href=\"./es/\"><svg>mark</svg></a>"
        );
        assert_eq!(
            out.get("es/index.html"),
            "<a href=\"../\"><svg>mark</svg></a>"
        );
    }

    #[test]
    fn makes_a_page_from_each_document() {
        let out = built(&repository());

        assert_eq!(
            out.get("philosophy.html"),
            "<title>Ideas &amp; &quot;rules&quot;</title><a href=\"./style.css\"><svg>mark</svg></a>\
             <main><h1 id=\"ideas-rules\">Ideas &amp; \"rules\"</h1>\n\
             <p>See <a href=\"decisions/server.html#d-002\">D-002</a>.</p>\n</main>\
             <a href=\"https://github.com/collide2954/post/blob/main/docs/philosophy.md\">source</a>"
        );
        for page in [
            "decisions/index.html",
            "decisions/product.html",
            "decisions/server.html",
            "protocol/index.html",
            "architecture.html",
            "threat-model.html",
            "verification.html",
            "changelog.html",
        ] {
            assert!(out.get(page).starts_with("<title>"), "{page}");
        }
    }

    #[test]
    fn connects_the_decision_records() {
        let out = built(&repository());

        let index = out.get("decisions/index.html");
        assert!(
            index.contains("<a href=\"product.html#d-001\">D-001</a> First"),
            "{index}"
        );
        assert!(index.contains("<a href=\"server.html#d-002\">D-002</a> Second"));
        assert!(index.contains("<a href=\"../style.css\">"));

        let product = out.get("decisions/product.html");
        assert!(product.contains("<a href=\"./\">guide</a>"), "{product}");
        assert!(product.contains("<h3 id=\"d-001\">D-001 First</h3>"));
        assert!(
            product.contains("blob/main/docs/decisions/product.md\">source</a>"),
            "{product}"
        );

        let changelog = out.get("changelog.html");
        assert!(changelog.contains("<a href=\"decisions/product.html#d-001\">D-001</a>"));

        let protocol = out.get("protocol/index.html");
        assert!(protocol.contains("tree/main/docs/protocol/schemas\">schemas</a>"));
    }

    #[test]
    fn reports_a_document_with_a_problem() {
        let root = repository();
        root.put(
            "docs/architecture.md",
            "# Architecture\n\n[gone](missing.md)\n",
        );

        let result = build(&root.0, &Directory::new().0);

        match result {
            Err(BuildError::Document { source, error }) => {
                assert_eq!(source, "docs/architecture.md");
                assert_eq!(error, RenderError::BrokenLink("missing.md".to_owned()));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn reports_a_file_that_is_missing() {
        let root = repository();
        fs::remove_file(root.0.join("CHANGELOG.md")).expect("a removed file");

        let result = build(&root.0, &Directory::new().0);

        match result {
            Err(BuildError::File { path, .. }) => assert!(path.ends_with("CHANGELOG.md")),
            other => panic!("{other:?}"),
        }
        let nothing = build(
            Path::new("post-site-missing-directory"),
            &Directory::new().0,
        );
        assert!(matches!(nothing, Err(BuildError::File { .. })));
    }

    #[test]
    fn reports_a_placeholder_that_it_does_not_know() {
        let root = repository();
        root.put("site/index.html", "{{title}}");

        let result = build(&root.0, &Directory::new().0);

        match result {
            Err(BuildError::Placeholder { page, name }) => {
                assert_eq!(page, "index.html");
                assert_eq!(name, "title");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn puts_each_value_in_its_place_one_time() {
        let values = [("a", "1{{b}}"), ("b", "2")];

        let filled = fill("{{a}} and {{b}}{{a}}.", &values, "page").expect("valid");

        assert_eq!(filled, "1{{b}} and 21{{b}}.");
        assert_eq!(fill("plain", &values, "page").expect("valid"), "plain");
    }

    #[test]
    fn refuses_a_placeholder_that_does_not_end() {
        let result = fill("start {{a", &[("a", "1")], "page");

        assert!(matches!(
            result,
            Err(BuildError::Placeholder { name, .. }) if name == "a"
        ));
    }

    #[test]
    fn makes_text_safe_for_a_page() {
        assert_eq!(
            escape("a < b & \"c\" > d"),
            "a &lt; b &amp; &quot;c&quot; &gt; d"
        );
    }

    #[test]
    fn describes_each_error() {
        let file = BuildError::File {
            path: PathBuf::from("x.md"),
            error: std::io::Error::other("no"),
        };
        assert_eq!(file.to_string(), "x.md: no");
        let document = BuildError::Document {
            source: "docs/a.md".to_owned(),
            error: RenderError::NoTitle,
        };
        assert_eq!(document.to_string(), "docs/a.md: the document has no title");
        let placeholder = BuildError::Placeholder {
            page: "index.html".to_owned(),
            name: "x".to_owned(),
        };
        assert_eq!(
            placeholder.to_string(),
            "index.html: the placeholder \"x\" is not known"
        );
    }
}
