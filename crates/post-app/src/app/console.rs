use std::collections::VecDeque;

use post_client::Line;

pub const MAX_LINES: usize = 200;

#[derive(Debug, Clone)]
pub enum Message {
    DraftChanged(String),
    Send,
    Clear,
}

#[derive(Debug, Default)]
pub struct Console {
    lines: VecDeque<Line>,
    draft: String,
    not_sent: bool,
}

impl Console {
    #[must_use]
    pub fn lines(&self) -> &VecDeque<Line> {
        &self.lines
    }

    #[must_use]
    pub fn draft(&self) -> &str {
        &self.draft
    }

    #[must_use]
    pub fn not_sent(&self) -> bool {
        self.not_sent
    }

    pub fn record(&mut self, line: Line) {
        if self.lines.len() == MAX_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    pub fn edit(&mut self, draft: String) {
        self.draft = draft;
        self.not_sent = false;
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn finish(&mut self, sent: bool) {
        if sent {
            self.draft.clear();
        }
        self.not_sent = !sent;
    }
}

#[cfg(test)]
mod tests {
    use post_client::{Direction, Line};
    use post_proto::Timestamp;

    use super::{Console, MAX_LINES};

    fn line(text: &str) -> Line {
        Line {
            time: Timestamp::parse("2026-11-21T14:02:07.120Z").expect("valid"),
            direction: Direction::Sent,
            text: text.to_owned(),
        }
    }

    #[test]
    fn starts_empty() {
        let console = Console::default();

        assert!(console.lines().is_empty());
        assert_eq!(console.draft(), "");
        assert!(!console.not_sent());
    }

    #[test]
    fn keeps_the_lines_in_sequence() {
        let mut console = Console::default();

        console.record(line("one"));
        console.record(line("two"));

        let texts: Vec<&str> = console
            .lines()
            .iter()
            .map(|line| line.text.as_str())
            .collect();
        assert_eq!(texts, ["one", "two"]);

        console.clear();
        assert!(console.lines().is_empty());
    }

    #[test]
    fn drops_the_oldest_line_when_it_is_full() {
        let mut console = Console::default();

        for number in 0..MAX_LINES + 2 {
            console.record(line(&number.to_string()));
        }

        assert_eq!(console.lines().len(), MAX_LINES);
        assert_eq!(console.lines()[0].text, "2");
        let newest = (MAX_LINES + 1).to_string();
        assert_eq!(console.lines()[MAX_LINES - 1].text, newest);
    }

    #[test]
    fn makes_the_field_empty_after_a_line_is_sent() {
        let mut console = Console::default();
        console.edit("{}".to_owned());
        assert_eq!(console.draft(), "{}");

        console.finish(true);

        assert_eq!(console.draft(), "");
        assert!(!console.not_sent());
    }

    #[test]
    fn keeps_the_text_of_a_line_that_is_not_sent() {
        let mut console = Console::default();
        console.edit("{}".to_owned());

        console.finish(false);

        assert_eq!(console.draft(), "{}");
        assert!(console.not_sent());

        console.edit("{ }".to_owned());
        assert!(!console.not_sent());
    }
}
