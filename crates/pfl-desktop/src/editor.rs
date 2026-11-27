//! Platform-independent UTF-8/grapheme editing, shared with the native IME handler.
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Revision {
    text: String,
    anchor: usize,
    head: usize,
}

/// The most characters one insertion may add.
///
/// A paste of a whole log file into a field is a mistake, not an edit; without a bound it makes the
/// layout and every redo snapshot expensive at once. The cut is made on a grapheme boundary.
pub const MAX_INSERT_CHARS: usize = 100_000;

#[derive(Clone, Debug)]
pub struct TextBuffer {
    current: Revision,
    undo: Vec<Revision>,
    redo: Vec<Revision>,
    marked: Option<Range<usize>>,
    read_only: bool,
    multiline: bool,
}

impl TextBuffer {
    pub fn new(text: impl Into<String>, multiline: bool) -> Self {
        Self {
            current: Revision {
                text: text.into(),
                anchor: 0,
                head: 0,
            },
            undo: Vec::new(),
            redo: Vec::new(),
            marked: None,
            read_only: false,
            multiline,
        }
    }

    pub fn read_only(text: impl Into<String>) -> Self {
        let mut buffer = Self::new(text, true);
        buffer.read_only = true;
        buffer
    }

    pub fn text(&self) -> &str {
        &self.current.text
    }
    pub fn cursor(&self) -> usize {
        self.current.head
    }
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }
    pub fn multiline(&self) -> bool {
        self.multiline
    }
    pub fn reversed(&self) -> bool {
        self.current.head < self.current.anchor
    }
    pub fn marked(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    pub fn selection(&self) -> Range<usize> {
        self.current.anchor.min(self.current.head)..self.current.anchor.max(self.current.head)
    }

    pub fn selected_text(&self) -> &str {
        &self.text()[self.selection()]
    }

    pub fn select(&mut self, anchor: usize, head: usize) {
        self.current.anchor = self.floor_boundary(anchor);
        self.current.head = self.floor_boundary(head);
    }

    pub fn move_to(&mut self, offset: usize, extend: bool) {
        let offset = self.floor_boundary(offset);
        if !extend {
            self.current.anchor = offset;
        }
        self.current.head = offset;
    }

    pub fn select_all(&mut self) {
        self.select(0, self.text().len());
    }

    fn floor_boundary(&self, offset: usize) -> usize {
        let offset = offset.min(self.text().len());
        self.text()
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .chain(std::iter::once(self.text().len()))
            .take_while(|index| *index <= offset)
            .last()
            .unwrap_or(0)
    }

    pub fn horizontal(&mut self, forward: bool, extend: bool) {
        let selection = self.selection();
        let next = if !extend && !selection.is_empty() {
            if forward {
                selection.end
            } else {
                selection.start
            }
        } else if forward {
            self.text()
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .find(|i| *i > self.cursor())
                .unwrap_or(self.text().len())
        } else {
            self.text()
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .take_while(|i| *i < self.cursor())
                .last()
                .unwrap_or(0)
        };
        self.move_to(next, extend);
    }

    pub fn line_edge(&mut self, end: bool, document: bool, extend: bool) {
        let offset = if document {
            if end { self.text().len() } else { 0 }
        } else if end {
            self.text()[self.cursor()..]
                .find('\n')
                .map(|i| self.cursor() + i)
                .unwrap_or(self.text().len())
        } else {
            self.text()[..self.cursor()]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0)
        };
        self.move_to(offset, extend);
    }

    pub fn vertical(&mut self, lines: isize, extend: bool) {
        let starts: Vec<usize> = std::iter::once(0)
            .chain(self.text().match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        let row = starts
            .partition_point(|start| *start <= self.cursor())
            .saturating_sub(1);
        let column = self.text()[starts[row]..self.cursor()]
            .graphemes(true)
            .count();
        let target = row.saturating_add_signed(lines).min(starts.len() - 1);
        let end = starts
            .get(target + 1)
            .map(|i| i - 1)
            .unwrap_or(self.text().len());
        let offset = self.text()[starts[target]..end]
            .grapheme_indices(true)
            .map(|(i, _)| starts[target] + i)
            .nth(column)
            .unwrap_or(end);
        self.move_to(offset, extend);
    }

    /// Truncates an insertion at a grapheme boundary.
    fn capped(&self, text: &str) -> String {
        if text.len() <= MAX_INSERT_CHARS {
            text.to_string()
        } else {
            text.graphemes(true).take(MAX_INSERT_CHARS).collect()
        }
    }

    fn remember(&mut self) {
        // Bound edit history without imposing a document-length limit.
        if self.undo.len() == 200 {
            self.undo.remove(0);
        }
        self.undo.push(self.current.clone());
        self.redo.clear();
    }

    pub fn replace_selection(&mut self, text: &str) {
        self.replace_utf16(None, text);
    }

    fn normalize(&self, text: &str) -> String {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        if self.multiline {
            text
        } else {
            text.replace('\n', " ")
        }
    }

    fn replace_bytes(&mut self, range: Range<usize>, text: &str) {
        self.current.text.replace_range(range.clone(), text);
        self.current.head = range.start + text.len();
        self.current.anchor = self.current.head;
    }

    /// Native ranges are UTF-16 code units; internal selections are UTF-8 bytes.
    pub fn replace_utf16(&mut self, range: Option<Range<usize>>, text: &str) {
        if self.read_only {
            return;
        }
        let range = range
            .map(|r| self.from_utf16(r))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection());
        // An empty replacement of an empty range changes nothing; remembering it would clear the
        // redo stack for no reason, which is what an IME cancel used to do.
        if self.marked.is_none() && range.is_empty() && text.is_empty() {
            return;
        }
        if self.marked.is_none() {
            self.remember();
        }
        let text = self.capped(&self.normalize(text));
        self.replace_bytes(range, &text);
        self.marked = None;
    }

    /// IME composition is one undo transaction, even after many candidate updates.
    pub fn replace_marked(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected_in_replacement: Option<Range<usize>>,
    ) {
        if self.read_only {
            return;
        }
        let range = range
            .map(|r| self.from_utf16(r))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection());
        if self.marked.is_none() {
            self.remember();
        }
        let text = self.normalize(text);
        self.replace_bytes(range.clone(), &text);
        self.marked = Some(range.start..range.start + text.len());
        if let Some(selected) = selected_in_replacement {
            let selected = utf16_range(&text, selected);
            self.current.anchor = range.start + selected.start;
            self.current.head = range.start + selected.end;
        }
    }

    pub fn unmark(&mut self) {
        self.marked = None;
    }

    pub fn delete(&mut self, forward: bool) {
        if self.read_only {
            return;
        }
        if self.selection().is_empty() {
            self.horizontal(forward, true);
        }
        if !self.selection().is_empty() {
            self.replace_selection("");
        }
    }

    pub fn undo(&mut self) {
        if self.read_only {
            return;
        }
        if let Some(previous) = self.undo.pop() {
            self.redo
                .push(std::mem::replace(&mut self.current, previous));
            self.marked = None;
        }
    }

    pub fn redo(&mut self) {
        if self.read_only {
            return;
        }
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.current, next));
            self.marked = None;
        }
    }

    /// An intentional document load, not a user edit or an inference request.
    pub fn load(&mut self, text: impl Into<String>) {
        self.current = Revision {
            text: text.into(),
            anchor: 0,
            head: 0,
        };
        self.undo.clear();
        self.redo.clear();
        self.marked = None;
    }

    pub fn from_utf16(&self, range: Range<usize>) -> Range<usize> {
        utf16_range(self.text(), range)
    }

    pub fn to_utf16(&self, range: Range<usize>) -> Range<usize> {
        self.text()[..char_boundary(self.text(), range.start)]
            .encode_utf16()
            .count()
            ..self.text()[..char_boundary(self.text(), range.end)]
                .encode_utf16()
                .count()
    }
}

fn char_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// Malformed/split surrogate ranges are expanded, never sliced through UTF-8.
fn utf16_range(text: &str, range: Range<usize>) -> Range<usize> {
    let start_units = range.start.min(range.end);
    let end_units = range.end.max(range.start);
    let mut units = 0;
    let mut start = text.len();
    let mut end = text.len();
    let mut found_start = false;
    for (byte, ch) in text.char_indices() {
        if !found_start && units + ch.len_utf16() > start_units {
            start = byte;
            found_start = true;
        }
        if units >= end_units {
            end = byte;
            break;
        }
        units += ch.len_utf16();
    }
    // A caret inside a surrogate pair stays a caret before that character.
    if start_units == end_units {
        return start..start;
    }
    start..end.max(start)
}

/// What Copy or Cut may put on the clipboard.
///
/// `None` for a masked field, whatever is selected: a session-only key on the clipboard, and from
/// there into clipboard history, is the leak the mask exists to prevent. `None` for an empty
/// selection too, so the clipboard is not silently emptied.
pub fn clipboard_payload(masked: bool, selection: &str) -> Option<String> {
    if masked || selection.is_empty() {
        None
    } else {
        Some(selection.to_string())
    }
}

#[cfg(test)]
mod clipboard_tests {
    use super::*;
    use unicode_segmentation::UnicodeSegmentation;

    #[test]
    fn a_masked_field_never_reaches_the_clipboard() {
        assert_eq!(clipboard_payload(true, "sk-ant-session-only"), None);
    }

    #[test]
    fn an_empty_selection_copies_nothing() {
        assert_eq!(clipboard_payload(false, ""), None);
    }

    #[test]
    fn an_ordinary_selection_is_copied_verbatim() {
        assert_eq!(
            clipboard_payload(false, "invention notes café"),
            Some("invention notes café".to_string())
        );
    }

    #[test]
    fn a_paste_is_capped_at_a_grapheme_boundary() {
        let mut buffer = TextBuffer::new("", true);
        buffer.replace_utf16(None, &"é".repeat(MAX_INSERT_CHARS + 5_000));
        assert_eq!(
            buffer.text().graphemes(true).count(),
            MAX_INSERT_CHARS,
            "a huge paste is truncated, not accepted whole"
        );
    }

    #[test]
    fn an_empty_replacement_keeps_the_redo_stack() {
        let mut buffer = TextBuffer::new("", true);
        buffer.replace_utf16(None, "hello");
        buffer.undo();
        assert_eq!(buffer.text(), "");
        // An IME cancel with nothing to replace must not count as an edit.
        buffer.replace_utf16(None, "");
        buffer.redo();
        assert_eq!(
            buffer.text(),
            "hello",
            "redo must survive an empty replacement"
        );
    }
}
