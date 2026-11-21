pub const REPOSITORY: &str = "https://github.com/collide2954/post";
const INDEX: &str = "index.html";

#[must_use]
pub fn resolve(source: &str, link: &str) -> Option<String> {
    let mut parts: Vec<&str> = source.split('/').collect();
    parts.pop();
    for part in link.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(name),
        }
    }
    Some(parts.join("/"))
}

#[must_use]
pub fn relative(from: &str, to: &str) -> String {
    let (from_directories, _) = split(from);
    let (to_directories, to_file) = split(to);
    let common = from_directories
        .iter()
        .zip(&to_directories)
        .take_while(|(from, to)| from == to)
        .count();

    let mut url = "../".repeat(from_directories.len() - common);
    for directory in &to_directories[common..] {
        url.push_str(directory);
        url.push('/');
    }
    if to_file != INDEX {
        url.push_str(to_file);
    }
    if url.is_empty() {
        url.push_str("./");
    }
    url
}

#[must_use]
pub fn root(page: &str) -> String {
    let depth = split(page).0.len();
    if depth == 0 {
        "./".to_owned()
    } else {
        "../".repeat(depth)
    }
}

fn split(path: &str) -> (Vec<&str>, &str) {
    let mut directories: Vec<&str> = path.split('/').collect();
    let file = directories.pop().unwrap_or_default();
    (directories, file)
}

#[cfg(test)]
mod tests {
    use super::{relative, resolve, root};

    #[test]
    fn joins_a_link_to_the_directory_of_its_document() {
        for (source, link, path) in [
            (
                "docs/philosophy.md",
                "decisions/README.md",
                "docs/decisions/README.md",
            ),
            (
                "docs/decisions/engineering.md",
                "README.md",
                "docs/decisions/README.md",
            ),
            (
                "docs/decisions/README.md",
                "../philosophy.md",
                "docs/philosophy.md",
            ),
            (
                "docs/protocol/README.md",
                "schemas",
                "docs/protocol/schemas",
            ),
            (
                "docs/protocol/README.md",
                "./examples/errors.txt",
                "docs/protocol/examples/errors.txt",
            ),
            (
                "docs/protocol/README.md",
                "schemas/",
                "docs/protocol/schemas",
            ),
            ("CHANGELOG.md", "docs/philosophy.md", "docs/philosophy.md"),
            (
                "docs/architecture.md",
                "../crates/post-proto",
                "crates/post-proto",
            ),
            ("docs/architecture.md", "..", ""),
        ] {
            assert_eq!(
                resolve(source, link).as_deref(),
                Some(path),
                "{source} {link}"
            );
        }
    }

    #[test]
    fn refuses_a_link_that_goes_out_of_the_repository() {
        assert_eq!(resolve("docs/philosophy.md", "../../secret"), None);
        assert_eq!(resolve("CHANGELOG.md", "../post"), None);
    }

    #[test]
    fn gives_the_address_of_one_page_from_a_different_page() {
        for (from, to, url) in [
            ("philosophy.html", "architecture.html", "architecture.html"),
            ("philosophy.html", "decisions/index.html", "decisions/"),
            (
                "philosophy.html",
                "decisions/product.html",
                "decisions/product.html",
            ),
            ("philosophy.html", "index.html", "./"),
            (
                "decisions/engineering.html",
                "philosophy.html",
                "../philosophy.html",
            ),
            ("decisions/engineering.html", "decisions/index.html", "./"),
            (
                "decisions/index.html",
                "decisions/product.html",
                "product.html",
            ),
            (
                "decisions/product.html",
                "protocol/index.html",
                "../protocol/",
            ),
            ("decisions/product.html", "index.html", "../"),
            ("es/index.html", "index.html", "../"),
            ("index.html", "es/index.html", "es/"),
            ("a/b/c.html", "a/d/e.html", "../d/e.html"),
        ] {
            assert_eq!(relative(from, to), url, "{from} {to}");
        }
    }

    #[test]
    fn gives_the_way_from_a_page_to_the_top_of_the_website() {
        assert_eq!(root("index.html"), "./");
        assert_eq!(root("decisions/index.html"), "../");
        assert_eq!(root("a/b/c.html"), "../../");
    }
}
