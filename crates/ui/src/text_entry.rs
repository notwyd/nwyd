use crate::widget::EditState;

#[derive(Debug, Clone, PartialEq)]
pub struct Preedit {
    text: String,
    cursor: Option<(usize, usize)>,
    baseline: (String, i32, i32),
}

pub fn preedit(edit: &mut EditState, text: &str, cursor: Option<(usize, usize)>) {
    edit.preedit = (!text.is_empty()).then(|| Preedit {
        text: text.to_owned(),
        cursor,
        baseline: (edit.text.clone(), edit.caret, edit.anchor),
    });
}

pub fn presentation(edit: &EditState) -> std::borrow::Cow<'_, EditState> {
    let Some(preedit) = &edit.preedit else {
        return std::borrow::Cow::Borrowed(edit);
    };

    if preedit.baseline.0 != edit.text
        || preedit.baseline.1 != edit.caret
        || preedit.baseline.2 != edit.anchor
    {
        return std::borrow::Cow::Borrowed(edit);
    }

    let mut shown = edit.clone();

    shown.preedit = None;

    let mut text = chars(edit);

    let (start, end) = span(edit, text.len());

    text.splice(start..end, preedit.text.chars());

    shown.text = text.into_iter().collect();

    let index = |offset: usize| {
        let mut byte = offset.min(preedit.text.len());

        while !preedit.text.is_char_boundary(byte) {
            byte -= 1;
        }

        start
            .saturating_add(preedit.text[..byte].chars().count())
            .min(i32::MAX as usize) as i32
    };

    if let Some((a, b)) = preedit.cursor {
        shown.caret = index(a);

        shown.anchor = index(b);

        shown.caret_visible = true;
    } else {
        shown.caret = index(preedit.text.len());

        shown.anchor = shown.caret;

        shown.caret_suppressed = true;
    }

    std::borrow::Cow::Owned(shown)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Tab,
    Return,
    Escape,
    PageUp,
    PageDown,
    End,
    Home,
    Left,
    Up,
    Right,
    Down,
    Insert,
    Delete,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Raised {
    Submit = 0x601,
    Changed = 0x602,
    HistoryUp = 0x605,
    HistoryDown = 0x606,
    Backspaced = 0x607,
    FocusNext = 0x608,
    PageUp = 0x609,
    PageDown = 0x610,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Limits {
    pub characters: i32,
}

impl Limits {
    pub fn of(edit: &EditState) -> Limits {
        Limits {
            characters: if edit.range <= 0 && edit.password {
                11
            } else {
                edit.range
            },
        }
    }
}

pub trait Clipboard {
    fn get(&self) -> String;

    fn set(&mut self, text: &str);
}

#[derive(Debug, Clone, Default)]
pub struct Local(pub String);

impl Clipboard for Local {
    fn get(&self) -> String {
        self.0.clone()
    }

    fn set(&mut self, text: &str) {
        self.0 = text.to_string();
    }
}

fn span(edit: &EditState, len: usize) -> (usize, usize) {
    let clamp = |v: i32| v.clamp(0, len as i32) as usize;

    let (a, b) = (clamp(edit.caret), clamp(edit.anchor));

    (a.min(b), a.max(b))
}

fn chars(edit: &EditState) -> Vec<char> {
    edit.text.chars().collect()
}

fn store(edit: &mut EditState, text: Vec<char>, caret: usize) {
    edit.reset_caret();

    edit.text = text.into_iter().collect();

    edit.caret = caret as i32;

    edit.anchor = caret as i32;
}

fn delete_selection(edit: &mut EditState) -> bool {
    let mut text = chars(edit);

    let (start, end) = span(edit, text.len());

    if start == end {
        return false;
    }

    text.drain(start..end);

    store(edit, text, start);

    true
}

pub fn selected(edit: &EditState) -> String {
    let text = chars(edit);

    let (start, end) = span(edit, text.len());

    text[start..end].iter().collect()
}

fn insert(edit: &mut EditState, added: &str, limits: Limits) -> bool {
    insert_with_overwrite(edit, added, limits, !edit.insert)
}

fn insert_with_overwrite(
    edit: &mut EditState,
    added: &str,
    limits: Limits,
    overwrite: bool,
) -> bool {
    delete_selection(edit);

    let mut text = chars(edit);

    let caret = (edit.caret.max(0) as usize).min(text.len());

    let room = if limits.characters > 0 {
        (limits.characters as usize)
            .saturating_sub(text.len())
            .saturating_add(if overwrite {
                text.len().saturating_sub(caret)
            } else {
                0
            })
    } else {
        usize::MAX
    };

    let taken: Vec<char> = added.chars().take(room).collect();

    if taken.is_empty() {
        return false;
    }

    let n = taken.len();

    if overwrite {
        let end = (caret + n).min(text.len());

        text.drain(caret..end);
    }

    text.splice(caret..caret, taken);

    store(edit, text, caret + n);

    true
}

pub fn character(edit: &mut EditState, ch: char, clipboard: &mut dyn Clipboard) -> Vec<Raised> {
    edit.preedit = None;

    if edit.pad.as_ref().is_some_and(|pad| pad.read_only) {
        return Vec::new();
    }

    let limits = Limits::of(edit);

    match ch {
        '\u{1}' => {
            let len = edit.text.chars().count() as i32;

            edit.anchor = 0;

            edit.caret = len;

            Vec::new()
        }
        '\u{3}' => {
            if edit.caret != edit.anchor {
                clipboard.set(&selected(edit));
            }

            Vec::new()
        }
        '\u{8}' => {
            edit.reset_caret();

            if delete_selection(edit) {
                return vec![Raised::Changed, Raised::Backspaced];
            }

            let mut text = chars(edit);

            let caret = (edit.caret.max(0) as usize).min(text.len());

            if caret == 0 {
                return Vec::new();
            }

            text.remove(caret - 1);

            store(edit, text, caret - 1);

            vec![Raised::Changed, Raised::Backspaced]
        }
        '\u{16}' => {
            let pasted = clipboard.get();

            if insert_with_overwrite(edit, &pasted, limits, false) {
                vec![Raised::Changed]
            } else {
                Vec::new()
            }
        }
        '\u{18}' => {
            if edit.caret != edit.anchor {
                clipboard.set(&selected(edit));
            }

            if delete_selection(edit) {
                vec![Raised::Changed]
            } else {
                Vec::new()
            }
        }
        c if (c as u32) < 0x20 => Vec::new(),
        c => {
            if edit.num_attrib && !c.is_ascii_digit() {
                return Vec::new();
            }

            edit.reset_caret();

            if insert(edit, &c.to_string(), limits) {
                vec![Raised::Changed]
            } else {
                Vec::new()
            }
        }
    }
}

pub fn key(
    edit: &mut EditState,
    key: Key,
    mods: Mods,
    clipboard: &mut dyn Clipboard,
) -> Vec<Raised> {
    edit.preedit = None;

    let len = edit.text.chars().count() as i32;

    if edit.pad.as_ref().is_some_and(|pad| pad.read_only)
        && (key == Key::Delete || key == Key::Return || (key == Key::Insert && mods.shift))
    {
        return Vec::new();
    }

    if edit.note_pad && key == Key::Return {
        return if insert_with_overwrite(edit, "\n", Limits::of(edit), false) {
            vec![Raised::Changed]
        } else {
            Vec::new()
        };
    }

    let move_caret = |edit: &mut EditState, to: i32| {
        edit.reset_caret();

        edit.caret = to.clamp(0, len);

        if !mods.shift {
            edit.anchor = edit.caret;
        }
    };

    match key {
        Key::Tab => vec![Raised::FocusNext],
        Key::Return => vec![Raised::Submit],
        Key::Escape => Vec::new(),
        Key::PageUp => vec![Raised::PageUp],
        Key::PageDown => vec![Raised::PageDown],
        Key::Up => vec![Raised::HistoryUp],
        Key::Down => vec![Raised::HistoryDown],
        Key::Home => {
            move_caret(edit, 0);

            Vec::new()
        }
        Key::End => {
            move_caret(edit, len);

            Vec::new()
        }
        Key::Left => {
            let to = if mods.ctrl {
                word_left(edit)
            } else {
                edit.caret - 1
            };

            move_caret(edit, to);

            Vec::new()
        }
        Key::Right => {
            let to = if mods.ctrl {
                word_right(edit)
            } else {
                edit.caret + 1
            };

            move_caret(edit, to);

            Vec::new()
        }
        Key::Insert => {
            if mods.ctrl {
                if edit.caret != edit.anchor {
                    clipboard.set(&selected(edit));
                }

                Vec::new()
            } else if mods.shift {
                let pasted = clipboard.get();

                let limits = Limits::of(edit);

                if insert_with_overwrite(edit, &pasted, limits, false) {
                    vec![Raised::Changed]
                } else {
                    Vec::new()
                }
            } else {
                edit.insert = !edit.insert;

                Vec::new()
            }
        }
        Key::Delete => {
            edit.reset_caret();

            if delete_selection(edit) {
                return vec![Raised::Changed];
            }

            let mut text = chars(edit);

            let caret = (edit.caret.max(0) as usize).min(text.len());

            if caret >= text.len() {
                return Vec::new();
            }

            text.remove(caret);

            store(edit, text, caret);

            vec![Raised::Changed]
        }
    }
}

fn word_left(edit: &EditState) -> i32 {
    let text = chars(edit);

    let mut i = (edit.caret.max(0) as usize).min(text.len());

    while i > 0 && text[i - 1].is_whitespace() {
        i -= 1;
    }

    while i > 0 && !text[i - 1].is_whitespace() {
        i -= 1;
    }

    i as i32
}

fn word_right(edit: &EditState) -> i32 {
    let text = chars(edit);

    let mut i = (edit.caret.max(0) as usize).min(text.len());

    while i < text.len() && !text[i].is_whitespace() {
        i += 1;
    }

    while i < text.len() && text[i].is_whitespace() {
        i += 1;
    }

    i as i32
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::Widget;
    use formats::ui::pane::ElementType;

    fn editor(kind: ElementType, text: &str) -> EditState {
        let Widget::Edit(mut edit) = Widget::new(kind) else {
            panic!("expected edit widget")
        };

        edit.text = text.into();

        edit.caret = text.chars().count() as i32;

        edit.anchor = edit.caret;

        *edit
    }

    #[test]
    fn replacing_and_backspacing_selection_uses_unicode_character_indices() {
        let mut edit = editor(ElementType::EditBox, "aé🦀z");

        edit.caret = 3;

        edit.anchor = 1;

        assert_eq!(selected(&edit), "é🦀");

        assert_eq!(
            character(&mut edit, '界', &mut Local::default()),
            vec![Raised::Changed]
        );

        assert_eq!((&*edit.text, edit.caret, edit.anchor), ("a界z", 2, 2));

        assert_eq!(
            character(&mut edit, '\u{8}', &mut Local::default()),
            vec![Raised::Changed, Raised::Backspaced]
        );

        assert_eq!((&*edit.text, edit.caret, edit.anchor), ("az", 1, 1));
    }

    #[test]
    fn paste_inserts_even_in_overwrite_mode_and_limits_characters_not_bytes() {
        let mut edit = editor(ElementType::EditBox, "ab");

        edit.caret = 1;

        edit.anchor = 1;

        edit.range = 4;

        edit.insert = false;

        let mut clipboard = Local("é🦀xyz".into());

        assert_eq!(
            character(&mut edit, '\u{16}', &mut clipboard),
            vec![Raised::Changed]
        );

        assert_eq!((&*edit.text, edit.caret, edit.anchor), ("aé🦀b", 3, 3));

        assert!(character(&mut edit, '\u{16}', &mut clipboard).is_empty());

        assert_eq!(edit.text, "aé🦀b");

        edit.anchor = 1;

        character(&mut edit, '\u{3}', &mut clipboard);

        assert_eq!(clipboard.0, "é🦀");

        assert_eq!(
            character(&mut edit, '\u{18}', &mut clipboard),
            vec![Raised::Changed]
        );

        assert_eq!(edit.text, "ab");
    }

    #[test]
    fn overwrite_can_replace_a_character_at_the_length_limit() {
        let mut edit = editor(ElementType::EditBox, "abc");

        edit.range = 3;

        edit.caret = 1;

        edit.anchor = 1;

        key(
            &mut edit,
            Key::Insert,
            Mods::default(),
            &mut Local::default(),
        );

        assert!(!edit.insert);

        assert_eq!(
            character(&mut edit, '界', &mut Local::default()),
            vec![Raised::Changed]
        );

        assert_eq!((&*edit.text, edit.caret, edit.anchor), ("a界c", 2, 2));
    }

    #[test]
    fn word_selection_and_navigation_preserve_editing_boundaries() {
        let mut edit = editor(ElementType::EditBox, "ab cd");

        let mut clipboard = Local::default();

        let extend_word = Mods {
            ctrl: true,
            shift: true,
            ..Mods::default()
        };

        assert!(key(&mut edit, Key::Left, extend_word, &mut clipboard).is_empty());

        assert_eq!((edit.caret, edit.anchor), (3, 5));

        assert_eq!(selected(&edit), "cd");

        key(&mut edit, Key::Home, Mods::default(), &mut clipboard);

        assert_eq!((edit.caret, edit.anchor), (0, 0));

        assert!(character(&mut edit, '\u{8}', &mut clipboard).is_empty());

        key(&mut edit, Key::Left, Mods::default(), &mut clipboard);

        assert_eq!(edit.caret, 0);

        key(&mut edit, Key::End, Mods::default(), &mut clipboard);

        key(&mut edit, Key::Right, Mods::default(), &mut clipboard);

        assert_eq!(edit.caret, 5);

        assert!(key(&mut edit, Key::Delete, Mods::default(), &mut clipboard).is_empty());

        assert_eq!(
            key(&mut edit, Key::Return, Mods::default(), &mut clipboard),
            vec![Raised::Submit]
        );

        assert_eq!(
            key(&mut edit, Key::Tab, Mods::default(), &mut clipboard),
            vec![Raised::FocusNext]
        );

        assert_eq!(edit.text, "ab cd");
    }

    #[test]
    fn ime_presentation_handles_utf8_cursor_offsets_without_committing_text() {
        let mut edit = editor(ElementType::ImeEditBox, "axz");

        edit.caret = 2;

        edit.anchor = 1;

        preedit(&mut edit, "é🦀", Some((1, 5)));

        let shown = presentation(&edit);

        assert_eq!((&*shown.text, shown.caret, shown.anchor), ("aé🦀z", 1, 2));

        assert_eq!((&*edit.text, edit.caret, edit.anchor), ("axz", 2, 1));

        edit.text = "changed".into();

        assert_eq!(presentation(&edit).text, "changed");

        preedit(&mut edit, "", None);

        assert!(edit.preedit.is_none());
    }

    #[test]
    fn read_only_notepad_rejects_mutation_but_allows_navigation() {
        let mut edit = editor(ElementType::ImeNotePad, "locked");

        edit.pad.as_mut().unwrap().read_only = true;

        let mut clipboard = Local("replacement".into());

        for ch in ['x', '\u{8}', '\u{16}', '\u{18}'] {
            assert!(character(&mut edit, ch, &mut clipboard).is_empty());
        }

        for key_code in [Key::Delete, Key::Return, Key::Insert] {
            assert!(
                key(
                    &mut edit,
                    key_code,
                    Mods {
                        shift: true,
                        ..Mods::default()
                    },
                    &mut clipboard
                )
                .is_empty()
            );
        }

        key(&mut edit, Key::Home, Mods::default(), &mut clipboard);

        assert_eq!((&*edit.text, edit.caret, edit.anchor), ("locked", 0, 0));
    }

    #[test]
    fn numeric_password_rejects_non_ascii_digits_and_honors_default_limit() {
        let mut edit = editor(ElementType::EditBox, "");

        edit.password = true;

        edit.num_attrib = true;

        let mut clipboard = Local::default();

        for ch in ['é', '９', '-'] {
            assert!(character(&mut edit, ch, &mut clipboard).is_empty());
        }

        for ch in "12345678901".chars() {
            assert_eq!(
                character(&mut edit, ch, &mut clipboard),
                vec![Raised::Changed]
            );
        }

        assert!(character(&mut edit, '2', &mut clipboard).is_empty());

        assert_eq!(edit.text, "12345678901");

        assert_eq!(edit.masked().chars().count(), 11);
    }
}
