pub const STYLE: &str = include_str!("../assets/invite.css");
pub const SCRIPT: &str = include_str!("../assets/invite.js");
pub const POLICY: &str = "default-src 'none'; style-src 'self'; script-src 'self'; \
     base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
const DOWNLOADS: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/releases");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Spanish,
}

struct Texts {
    tag: &'static str,
    title: &'static str,
    lead: &'static str,
    steps: [&'static str; 3],
    copy: &'static str,
    copied: &'static str,
    question: &'static str,
    downloads: &'static str,
    not_valid: &'static str,
}

const ENGLISH: Texts = Texts {
    tag: "en",
    title: "Invitation to a Post server",
    lead: "This link is an invitation to a Post server.",
    steps: [
        "Copy the link.",
        "Start Post.",
        "Paste the link into the first field, and type a name.",
    ],
    copy: "Copy",
    copied: "Copied",
    question: "Do you not have Post?",
    downloads: "Get Post",
    not_valid: "This invitation is not valid. Ask for a new invite link.",
};

const SPANISH: Texts = Texts {
    tag: "es-419",
    title: "Invitación a un servidor de Post",
    lead: "Este enlace es una invitación a un servidor de Post.",
    steps: [
        "Copia el enlace.",
        "Inicia Post.",
        "Pega el enlace en el primer campo y escribe un nombre.",
    ],
    copy: "Copiar",
    copied: "Copiado",
    question: "¿No tienes Post?",
    downloads: "Descarga Post",
    not_valid: "Esta invitación no es válida. Pide un enlace de invitación nuevo.",
};

impl Language {
    #[must_use]
    pub fn from_header(header: Option<&str>) -> Self {
        let mut best = (Self::English, 0.0);
        for range in header.unwrap_or_default().split(',') {
            let mut parts = range.split(';');
            let tag = parts.next().unwrap_or_default().trim();
            let quality = parts
                .find_map(|part| part.trim().strip_prefix("q="))
                .map_or(Some(1.0), |quality| quality.parse::<f32>().ok())
                .filter(|quality| (0.0..=1.0).contains(quality));
            let primary = tag.split('-').next().unwrap_or_default();
            let language = if primary.eq_ignore_ascii_case("es") {
                Self::Spanish
            } else if primary.eq_ignore_ascii_case("en") {
                Self::English
            } else {
                continue;
            };
            if let Some(quality) = quality
                && quality > best.1
            {
                best = (language, quality);
            }
        }
        best.0
    }

    fn texts(self) -> &'static Texts {
        match self {
            Self::English => &ENGLISH,
            Self::Spanish => &SPANISH,
        }
    }
}

#[must_use]
pub fn invitation(language: Language, link: &str) -> String {
    let texts = language.texts();
    let [first, second, third] = texts.steps;
    let body = format!(
        "<p>{lead}</p>\n\
         <ol>\n<li>{first}</li>\n<li>{second}</li>\n<li>{third}</li>\n</ol>\n\
         <p class=\"link\"><input id=\"link\" readonly value=\"{link}\">\
         <button id=\"copy\" type=\"button\" data-done=\"{copied}\">{copy}</button></p>\n\
         <p class=\"note\">{question} <a href=\"{DOWNLOADS}\" rel=\"noreferrer\">{downloads}</a>.</p>\n\
         <script src=\"/assets/invite.js\"></script>",
        lead = texts.lead,
        link = escape(link),
        copied = texts.copied,
        copy = texts.copy,
        question = texts.question,
        downloads = texts.downloads,
    );
    page(texts, &body)
}

#[must_use]
pub fn not_valid(language: Language) -> String {
    let texts = language.texts();
    page(texts, &format!("<p>{}</p>", texts.not_valid))
}

fn page(texts: &Texts, body: &str) -> String {
    format!(
        "<!doctype html>\n\
         <html lang=\"{tag}\">\n\
         <head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta name=\"robots\" content=\"noindex\">\n\
         <title>{title}</title>\n\
         <link rel=\"stylesheet\" href=\"/assets/invite.css\">\n\
         </head>\n\
         <body>\n\
         <main>\n\
         <h1>Post</h1>\n\
         {body}\n\
         </main>\n\
         </body>\n\
         </html>\n",
        tag = texts.tag,
        title = texts.title,
    )
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::{Language, POLICY, SCRIPT, STYLE, escape, invitation, not_valid};

    const LINK: &str = "https://chat.example.org/i/7Hq2kdX9mPq4";

    #[test]
    fn selects_the_language_that_the_browser_prefers() {
        for (header, language) in [
            (None, Language::English),
            (Some(""), Language::English),
            (Some("en"), Language::English),
            (Some("es"), Language::Spanish),
            (Some("ES-ar"), Language::Spanish),
            (Some("es-MX,es;q=0.9,en;q=0.8"), Language::Spanish),
            (Some("en-US,en;q=0.9,es;q=0.8"), Language::English),
            (Some("en;q=0.2, es;q=0.9"), Language::Spanish),
            (Some("fr,es;q=0.5"), Language::Spanish),
            (Some("fr-CA, de;q=0.8"), Language::English),
            (Some("es;q=0,en;q=0.1"), Language::English),
            (Some("es;q=0"), Language::English),
            (Some("es;q=many"), Language::English),
            (Some("es;q=7"), Language::English),
            (Some("*"), Language::English),
            (Some("est,eo"), Language::English),
        ] {
            assert_eq!(Language::from_header(header), language, "{header:?}");
        }
    }

    #[test]
    fn shows_the_link_and_the_steps_in_english() {
        let page = invitation(Language::English, LINK);

        assert!(page.starts_with("<!doctype html>\n<html lang=\"en\">"));
        for part in [
            "<title>Invitation to a Post server</title>",
            "<p>This link is an invitation to a Post server.</p>",
            "<li>Copy the link.</li>",
            "<li>Paste the link into the first field, and type a name.</li>",
            "<input id=\"link\" readonly value=\"https://chat.example.org/i/7Hq2kdX9mPq4\">",
            "<button id=\"copy\" type=\"button\" data-done=\"Copied\">Copy</button>",
            "Do you not have Post? <a href=\"https://github.com/collide2954/post/releases\" rel=\"noreferrer\">Get Post</a>.",
        ] {
            assert!(page.contains(part), "{part}\n{page}");
        }
    }

    #[test]
    fn shows_the_page_in_latin_american_spanish() {
        let page = invitation(Language::Spanish, LINK);

        for part in [
            "<html lang=\"es-419\">",
            "<title>Invitación a un servidor de Post</title>",
            "<li>Pega el enlace en el primer campo y escribe un nombre.</li>",
            "data-done=\"Copiado\">Copiar</button>",
            "¿No tienes Post?",
            LINK,
        ] {
            assert!(page.contains(part), "{part}\n{page}");
        }
    }

    #[test]
    fn tells_that_an_invitation_is_not_valid() {
        let english = not_valid(Language::English);
        let spanish = not_valid(Language::Spanish);

        assert!(
            english.contains("<p>This invitation is not valid. Ask for a new invite link.</p>")
        );
        assert!(
            spanish.contains(
                "<p>Esta invitación no es válida. Pide un enlace de invitación nuevo.</p>"
            )
        );
        for page in [english, spanish] {
            assert!(!page.contains("<input"));
            assert!(!page.contains("<script"));
        }
    }

    #[test]
    fn has_no_code_and_no_style_in_the_page_itself() {
        for page in [
            invitation(Language::English, LINK),
            not_valid(Language::English),
        ] {
            assert!(!page.contains("<style"), "{page}");
            assert!(!page.contains("style="), "{page}");
            assert!(!page.contains("onclick"), "{page}");
            assert!(!page.contains("<script>"), "{page}");
            assert!(page.contains("<link rel=\"stylesheet\" href=\"/assets/invite.css\">"));
        }
        assert!(POLICY.contains("default-src 'none'"));
        assert!(POLICY.contains("script-src 'self'"));
        assert!(!POLICY.contains("unsafe"));
        assert!(SCRIPT.contains("clipboard"));
        assert!(STYLE.contains("prefers-color-scheme"));
    }

    #[test]
    fn writes_special_characters_of_a_link_as_entities() {
        assert_eq!(
            escape("https://a.example/i/x?<b>&\"c\"='d'"),
            "https://a.example/i/x?&lt;b&gt;&amp;&quot;c&quot;=&#39;d&#39;"
        );

        let page = invitation(Language::English, "\"><script>alert(1)</script>");

        assert!(!page.contains("<script>alert"));
        assert!(page.contains("value=\"&quot;&gt;&lt;script&gt;alert(1)&lt;/script&gt;\""));
    }
}
