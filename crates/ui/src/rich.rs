use crate::layout::{MAX_ITERATIONS, Measure, Truncation};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Style {
    pub color: Option<u32>,
    pub shadow: Option<u32>,
    pub font: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub style: Style,
    pub width: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line {
    pub runs: Vec<Run>,
    pub width: i32,
    pub height: i32,
}

impl Line {
    pub fn text(&self) -> String {
        self.runs.iter().map(|run| run.text.as_str()).collect()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Layout {
    pub lines: Vec<Line>,
    pub truncated: Option<Truncation>,
}

impl Layout {
    pub fn total_height(&self) -> i32 {
        self.lines
            .iter()
            .fold(0i32, |height, line| height.saturating_add(line.height))
    }
}

enum Kind {
    Unit(u16),
    Break,
    Boundary,
    Pass,
}

struct Token {
    kind: Kind,
    style: Style,
    at: usize,
}

#[derive(Default)]
struct Tags {
    colors: Vec<u32>,
    shadows: Vec<u32>,
    fonts: Vec<i32>,
    forced: Option<u32>,
}

impl Tags {
    fn style(&self) -> Style {
        Style {
            color: self.forced.or_else(|| self.colors.last().copied()),
            shadow: self.shadows.last().copied(),
            font: self.fonts.last().copied(),
        }
    }

    fn apply(&mut self, tag: &str) -> Option<Kind> {
        match tag {
            "<n>" => return Some(Kind::Break),
            "<pass>" | "</pass>" => return Some(Kind::Pass),
            "</color>" => {
                self.colors.pop();
            }
            "</shadow>" => {
                self.shadows.pop();
            }
            "</font>" => {
                self.fonts.pop();
            }
            "</fcolor>" => self.forced = None,
            _ => {
                if let Some(rgb) = rgb(tag, "color") {
                    self.colors.push(rgb);
                } else if let Some(rgb) = rgb(tag, "fcolor") {
                    self.forced = Some(rgb);
                } else if let Some(rgb) = rgb(tag, "shadow") {
                    self.shadows.push(rgb);
                } else {
                    let value = tag
                        .strip_prefix("<font ")
                        .and_then(|s| s.strip_suffix('>'))?;

                    let font = value.parse::<i32>().ok()?;

                    if !(0..10).contains(&font) || tag != format!("<font {font}>") {
                        return None;
                    }

                    self.fonts.push(font);
                }
            }
        }

        Some(Kind::Boundary)
    }
}

fn rgb(tag: &str, name: &str) -> Option<u32> {
    let body = tag.strip_prefix(&format!("<{name} "))?.strip_suffix('>')?;

    let mut numbers = body.split(' ');

    let r = numbers.next()?.parse::<u8>().ok()?;

    let g = numbers.next()?.parse::<u8>().ok()?;

    let b = numbers.next()?.parse::<u8>().ok()?;

    if numbers.next().is_some() || tag != format!("<{name} {r} {g} {b}>") {
        return None;
    }

    Some((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b))
}

struct Parser {
    units: Vec<u16>,
    index: usize,
    tags: Tags,
}

impl Parser {
    fn new(text: &str) -> Self {
        Self {
            units: text.encode_utf16().collect(),
            index: 0,
            tags: Tags::default(),
        }
    }

    fn next(&mut self) -> Option<Token> {
        let at = self.index;

        let unit = *self.units.get(at)?;

        if unit == b'<' as u16 {
            let end = (at + 32).min(self.units.len());

            if let Some(len) = self.units[at..end].iter().position(|u| *u == b'>' as u16)
                && let Ok(tag) = String::from_utf16(&self.units[at..=at + len])
                && let Some(kind) = self.tags.apply(&tag)
            {
                self.index += len + 1;

                return Some(Token {
                    kind,
                    style: self.tags.style(),
                    at,
                });
            }
        }

        self.index += 1;

        let kind = if unit == 13 && self.units.get(self.index) == Some(&10) {
            self.index += 1;

            Kind::Break
        } else if unit == 10 {
            Kind::Break
        } else {
            Kind::Unit(unit)
        };

        Some(Token {
            kind,
            style: self.tags.style(),
            at,
        })
    }
}

struct PendingRun {
    units: Vec<u16>,
    style: Style,
    width: i32,
}

#[derive(Default)]
struct PendingLine {
    runs: Vec<PendingRun>,
    width: i32,
    height: i32,
}

fn commit(layout: &mut Layout, pending: &mut PendingLine, gap: i32) {
    let pending = std::mem::take(pending);

    if let Some(previous) = layout.lines.last_mut() {
        previous.height = previous.height.saturating_add(gap);
    }

    layout.lines.push(Line {
        width: pending.width,
        height: pending.height,
        runs: pending
            .runs
            .into_iter()
            .map(|run| Run {
                text: String::from_utf16_lossy(&run.units),
                style: run.style,
                width: run.width,
            })
            .collect(),
    });
}

pub fn wrap(
    text: &str,
    font: i32,
    width: i32,
    gap: i32,
    measure: &mut dyn Measure,
) -> Option<Layout> {
    measure.height(font)?;

    let mut parser = Parser::new(text);

    let mut layout = Layout::default();

    let mut line = PendingLine::default();

    let mut retry = None;

    let mut boundary = true;

    let mut iterations = 0;

    let mut last_fail = 0;

    while let Some(token) = retry.take().or_else(|| parser.next()) {
        iterations += 1;

        if iterations > MAX_ITERATIONS {
            layout.truncated = Some(Truncation::IterationCap);

            return Some(layout);
        }

        match token.kind {
            Kind::Boundary => boundary = true,
            Kind::Pass => {}
            Kind::Break => {
                commit(&mut layout, &mut line, gap);

                boundary = true;
            }
            Kind::Unit(unit) => {
                let font = token.style.font.unwrap_or(font);

                let height = measure.height(font)?;

                let advance = measure.advance(font, unit).unwrap_or(0);

                if line.width.saturating_add(advance) >= width {
                    if last_fail == token.at {
                        layout.truncated = Some(Truncation::UnfittableUnit);

                        return Some(layout);
                    }

                    commit(&mut layout, &mut line, gap);

                    last_fail = token.at;

                    retry = Some(token);

                    boundary = true;

                    continue;
                }

                if boundary || line.runs.last().is_none_or(|run| run.style != token.style) {
                    line.runs.push(PendingRun {
                        units: Vec::new(),
                        style: token.style,
                        width: 0,
                    });
                }

                let run = line.runs.last_mut()?;

                run.units.push(unit);

                run.width = run.width.saturating_add(advance);

                line.width = line.width.saturating_add(advance);

                line.height = line.height.max(height);

                boundary = false;
            }
        }
    }

    commit(&mut layout, &mut line, gap);

    Some(layout)
}
