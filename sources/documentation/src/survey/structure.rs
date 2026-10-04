//! Reads a document's structure by hand — headings, fences, front matter,
//! and the blocks between them — for the outline the survey lays and, under
//! `anchors`, the lines a rule can be stated at.
//!
//! The reader is a line scanner over the Markdown and Gherkin the documents
//! an adapter meets are written in; it is not a parser, and what it cannot
//! place reads as a paragraph.

use emery_sdk::survey::Lines;

/// What one document's lines hold.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Outline {
    /// The document's line count.
    pub lines: u32,
    /// Every heading, in order.
    pub headings: Vec<Heading>,
    /// Every block, in order, each over the lines it spans.
    pub blocks: Vec<Block>,
}

/// One heading: an ATX or setext heading, or a Gherkin keyword line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Heading {
    /// The heading's level, `1` for a document's title.
    pub level: u8,
    /// The line it stands at, 1-based.
    pub line: u32,
    /// Its text, with the marks stripped.
    pub text: String,
}

/// One block of a document and the lines it spans.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Block {
    /// What the block is.
    pub kind: Kind,
    /// The lines it spans, inclusive.
    pub lines: Lines,
}

/// The kinds of block the reader tells apart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// Running prose, to the next blank line or block.
    Paragraph,
    /// One list item with its continuation lines.
    Item,
    /// One body row of a table.
    Row,
    /// One Gherkin step.
    Step,
    /// A blockquote, consecutive `>` lines together.
    Quote,
    /// A heading line, or a setext heading with its underline.
    Heading,
    /// A fenced code block, fence to fence.
    Fence,
    /// YAML front matter at the top of the document.
    FrontMatter,
    /// A thematic break, a table's header and delimiter rows, an HTML block.
    Other,
}

impl Kind {
    /// Whether a block of this kind can state a rule: prose a `requirement`
    /// may anchor at, as against a heading, a fence, or front matter.
    #[cfg(feature = "anchors")]
    #[must_use]
    pub const fn states(self) -> bool {
        matches!(self, Self::Paragraph | Self::Item | Self::Row | Self::Step | Self::Quote)
    }
}

impl Outline {
    /// Reads a document's structure.
    #[must_use]
    pub fn read(text: &str) -> Self {
        let lines: Vec<&str> = text.lines().collect();
        let mut reader = Reader {
            lines: &lines,
            gherkin: gherkin(&lines),
            blocks: Vec::new(),
            headings: Vec::new(),
            open: None,
        };
        reader.read();
        let mut blocks = reader.blocks;
        blocks.sort_by_key(|block| block.lines);
        Self {
            lines: line_number(lines.len()),
            headings: reader.headings,
            blocks,
        }
    }

    /// Whether the document holds a block of `kind`.
    #[must_use]
    pub fn holds(&self, kind: Kind) -> bool {
        self.blocks.iter().any(|block| block.kind == kind)
    }

    /// The heading standing at `line`, when one does.
    #[must_use]
    pub fn heading_at(&self, line: u32) -> Option<&Heading> {
        self.headings.iter().find(|heading| heading.line == line)
    }

    /// The spans within `span` where a rule can be stated, in order.
    #[cfg(feature = "anchors")]
    pub fn stating(&self, span: Lines) -> impl Iterator<Item = Lines> + '_ {
        self.blocks
            .iter()
            .filter(move |block| block.kind.states() && span.contains(block.lines))
            .map(|block| block.lines)
    }
}

/// The first non-blank line of a document and its number, when it has one.
#[must_use]
pub fn opens(text: &str) -> Option<(u32, &str)> {
    text.lines()
        .enumerate()
        .find(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| (line_number(index + 1), line.trim()))
}

fn line_number(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}

// A Gherkin document opens with `Feature:`, past any tags and comments.
fn gherkin(lines: &[&str]) -> bool {
    lines
        .iter()
        .map(|line| line.trim())
        .find(|line| !line.is_empty() && !line.starts_with('@') && !line.starts_with('#'))
        .is_some_and(|line| line.starts_with("Feature:"))
}

// A block being read, from its first line.
struct Open {
    kind: Kind,
    start: u32,
}

struct Reader<'a> {
    lines: &'a [&'a str],
    gherkin: bool,
    blocks: Vec<Block>,
    headings: Vec<Heading>,
    open: Option<Open>,
}

impl Reader<'_> {
    fn read(&mut self) {
        let mut index = self.front_matter();
        let mut table = false;
        while index < self.lines.len() {
            let line = self.lines[index];
            let number = line_number(index + 1);
            let trimmed = line.trim();

            if trimmed.is_empty() {
                self.close(number - 1);
                table = false;
                index += 1;
            } else if let Some(fence) = fence_mark(line) {
                self.close(number - 1);
                index = self.fence(index, fence);
            } else if let Some((level, text)) = self.heading_line(line) {
                self.close(number - 1);
                self.heading(level, number, number, text);
                index += 1;
            } else if table && line.contains('|') {
                self.block(Kind::Row, number, number);
                index += 1;
            } else if delimiter_row(line) && self.open_paragraph_row(number) {
                table = true;
                index += 1;
            } else if let Some(level) = setext_underline(line).filter(|_| self.open_paragraph()) {
                self.setext(level, number);
                index += 1;
            } else if thematic_break(trimmed) {
                self.close(number - 1);
                self.block(Kind::Other, number, number);
                index += 1;
            } else {
                self.text_line(line, number);
                index += 1;
            }
        }
        self.close(line_number(self.lines.len()));
    }

    fn heading_line<'l>(&self, line: &'l str) -> Option<(u8, &'l str)> {
        atx_heading(line).or_else(|| self.gherkin.then(|| gherkin_heading(line)).flatten())
    }

    // Front matter is a `---` on the first line through the next `---` or
    // `...` line; returns the index to read on from.
    fn front_matter(&mut self) -> usize {
        if self.lines.first().is_none_or(|line| line.trim_end() != "---") {
            return 0;
        }
        let close =
            self.lines.iter().enumerate().skip(1).find_map(|(index, line)| {
                matches!(line.trim_end(), "---" | "...").then_some(index)
            });
        let Some(close) = close else { return 0 };
        self.block(Kind::FrontMatter, 1, line_number(close + 1));
        close + 1
    }

    // A fence runs to the closing mark of the same character at least as
    // long, or to the end of the document; returns the index to read on from.
    fn fence(&mut self, open: usize, mark: &str) -> usize {
        let close = self.lines.iter().enumerate().skip(open + 1).find_map(|(index, line)| {
            let trimmed = line.trim();
            (trimmed.starts_with(mark) && trimmed.chars().all(|c| c == mark_char(mark)))
                .then_some(index)
        });
        let end = close.unwrap_or_else(|| self.lines.len().saturating_sub(1));
        self.block(Kind::Fence, line_number(open + 1), line_number(end + 1));
        end + 1
    }

    // A line of prose: a step, a quote line, a list item, or the start or
    // continuation of a paragraph, an item, or a quote.
    fn text_line(&mut self, line: &str, number: u32) {
        if self.gherkin && gherkin_step(line) {
            self.close(number - 1);
            self.block(Kind::Step, number, number);
        } else if line.trim_start().starts_with('>') {
            if !matches!(
                self.open,
                Some(Open {
                    kind: Kind::Quote,
                    ..
                })
            ) {
                self.close(number - 1);
                self.open = Some(Open {
                    kind: Kind::Quote,
                    start: number,
                });
            }
        } else if list_item(line) {
            self.close(number - 1);
            self.open = Some(Open {
                kind: Kind::Item,
                start: number,
            });
        } else if self.open.is_none() {
            let kind =
                if line.trim_start().starts_with('<') { Kind::Other } else { Kind::Paragraph };
            self.open = Some(Open { kind, start: number });
        }
        // otherwise the line continues the open paragraph, item, or quote
    }

    const fn open_paragraph(&self) -> bool {
        matches!(
            self.open,
            Some(Open {
                kind: Kind::Paragraph,
                ..
            })
        )
    }

    // A delimiter row under a one-line paragraph holding `|` makes that
    // paragraph a table's header: both become one `Other` block.
    fn open_paragraph_row(&mut self, number: u32) -> bool {
        let Some(Open {
            kind: Kind::Paragraph,
            start,
        }) = self.open
        else {
            return false;
        };
        if start + 1 != number || !self.line(start).contains('|') {
            return false;
        }
        self.open = None;
        self.block(Kind::Other, start, number);
        true
    }

    // The open paragraph becomes a heading of `level` with the underline.
    fn setext(&mut self, level: u8, number: u32) {
        let Some(Open { start, .. }) = self.open.take() else { return };
        let text = self.line(start).trim().to_owned();
        self.heading(level, start, number, &text);
    }

    fn heading(&mut self, level: u8, start: u32, end: u32, text: &str) {
        self.block(Kind::Heading, start, end);
        self.headings.push(Heading {
            level,
            line: start,
            text: text.to_owned(),
        });
    }

    fn line(&self, number: u32) -> &str {
        usize::try_from(number)
            .ok()
            .and_then(|number| self.lines.get(number.wrapping_sub(1)))
            .copied()
            .unwrap_or_default()
    }

    // Closes the open block at `end`, when one is open.
    fn close(&mut self, end: u32) {
        if let Some(Open { kind, start }) = self.open.take() {
            self.block(kind, start, end.max(start));
        }
    }

    fn block(&mut self, kind: Kind, start: u32, end: u32) {
        self.blocks.push(Block {
            kind,
            lines: Lines { start, end },
        });
    }
}

// Up to three spaces of indentation, then the rest.
fn unindented(line: &str) -> Option<&str> {
    let spaces = line.len() - line.trim_start_matches(' ').len();
    (spaces <= 3).then(|| &line[spaces..])
}

fn fence_mark(line: &str) -> Option<&str> {
    let rest = unindented(line)?;
    let mark = rest.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let run = rest.chars().take_while(|&c| c == mark).count();
    (run >= 3).then(|| &rest[..run])
}

fn mark_char(mark: &str) -> char {
    mark.chars().next().unwrap_or('`')
}

fn atx_heading(line: &str) -> Option<(u8, &str)> {
    let rest = unindented(line)?;
    let hashes = rest.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let after = &rest[hashes..];
    if !after.is_empty() && !after.starts_with([' ', '\t']) {
        return None;
    }
    let text = after.trim().trim_end_matches('#').trim();
    Some((u8::try_from(hashes).unwrap_or(6), text))
}

// `Feature:`, `Rule:`, `Background:`, `Scenario:`, `Scenario Outline:`,
// `Examples:` lines head what follows them as a heading does.
fn gherkin_heading(line: &str) -> Option<(u8, &str)> {
    let (keyword, text) = line.trim().split_once(':')?;
    let level = match keyword.trim() {
        "Feature" | "Ability" | "Business Need" => 1,
        "Rule" => 2,
        "Background" | "Scenario" | "Example" | "Scenario Outline" | "Scenario Template" => 3,
        "Examples" | "Scenarios" => 4,
        _ => return None,
    };
    Some((level, text.trim()))
}

fn gherkin_step(line: &str) -> bool {
    let trimmed = line.trim_start();
    ["Given ", "When ", "Then ", "And ", "But ", "* "]
        .iter()
        .any(|keyword| trimmed.starts_with(keyword))
}

fn list_item(line: &str) -> bool {
    let trimmed = line.trim_start();
    let bullet = trimmed
        .strip_prefix(['-', '*', '+'])
        .is_some_and(|rest| rest.starts_with([' ', '\t']) && !rest.trim().is_empty());
    let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
    let ordered = (1..=9).contains(&digits)
        && trimmed[digits..]
            .strip_prefix(['.', ')'])
            .is_some_and(|rest| rest.starts_with([' ', '\t']) && !rest.trim().is_empty());
    bullet || ordered
}

// `| --- | :---: |` and the like: every cell dashes, with optional colons.
fn delimiter_row(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.contains('-') || !trimmed.contains('|') {
        return false;
    }
    let inner = trimmed.trim_start_matches('|').trim_end_matches('|');
    inner.split('|').all(|cell| {
        let cell = cell.trim();
        cell.contains('-')
            && cell.trim_start_matches(':').trim_end_matches(':').chars().all(|c| c == '-')
    })
}

// `===` or `---` under a paragraph line; level 1 for `=`, 2 for `-`.
fn setext_underline(line: &str) -> Option<u8> {
    let rest = unindented(line)?.trim_end();
    let mark = rest.chars().next()?;
    if !matches!(mark, '=' | '-') || !rest.chars().all(|c| c == mark) {
        return None;
    }
    Some(if mark == '=' { 1 } else { 2 })
}

// `---`, `***`, `___`, spaces between allowed, three marks at the least.
fn thematic_break(trimmed: &str) -> bool {
    let mut marks = trimmed.chars().filter(|c| !c.is_whitespace());
    let Some(mark) = marks.next() else { return false };
    matches!(mark, '-' | '*' | '_')
        && marks.all(|c| c == mark)
        && trimmed.matches(mark).count() >= 3
}
