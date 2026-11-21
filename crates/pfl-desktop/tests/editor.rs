use pfl_desktop::editor::TextBuffer;

#[test]
fn grapheme_cursor_delete_and_undo_preserve_combining_and_emoji_sequences() {
    let mut text = TextBuffer::new("A👩‍🔬e\u{301}Z", true);
    text.move_to(text.text().len(), false);
    text.horizontal(false, false);
    assert_eq!(&text.text()[text.cursor()..], "Z");
    text.delete(false);
    assert_eq!(text.text(), "A👩‍🔬Z");
    text.delete(false);
    assert_eq!(text.text(), "AZ");
    text.undo();
    text.undo();
    assert_eq!(text.text(), "A👩‍🔬e\u{301}Z");
    text.redo();
    assert_eq!(text.text(), "A👩‍🔬Z");
}

#[test]
fn reversed_selection_replaces_exactly_selected_graphemes() {
    let mut text = TextBuffer::new("one 世界 three", true);
    let start = "one ".len();
    let end = "one 世界".len();
    text.select(end, start);
    assert!(text.reversed());
    assert_eq!(text.selected_text(), "世界");
    text.replace_selection("two");
    assert_eq!(text.text(), "one two three");
    text.undo();
    assert_eq!(text.selected_text(), "世界");
}

#[test]
fn utf16_ime_composition_is_one_undo_and_selection_is_relative_to_replacement() {
    let mut text = TextBuffer::new("prefix:", true);
    text.move_to(7, false);
    text.replace_marked(None, "ni", Some(0..2));
    text.replace_marked(None, "你", Some(1..1));
    assert_eq!(text.text(), "prefix:你");
    assert_eq!(text.cursor(), "prefix:你".len());
    text.replace_utf16(None, "你好");
    assert_eq!(text.text(), "prefix:你好");
    text.undo();
    assert_eq!(text.text(), "prefix:");
    text.redo();
    assert_eq!(text.text(), "prefix:你好");
}

#[test]
fn utf16_surrogate_boundaries_are_safe_and_roundtrip() {
    let mut text = TextBuffer::new("A🧪B", true);
    assert_eq!(text.from_utf16(1..3), 1..5);
    assert_eq!(text.to_utf16(1..5), 1..3);
    assert_eq!(text.from_utf16(2..2), 1..1);
    assert_eq!(text.from_utf16(2..3), 1..5);
    text.replace_utf16(Some(1..3), "é");
    assert_eq!(text.text(), "AéB");
    assert_eq!(text.from_utf16(999..999), text.text().len()..text.text().len());
}

#[test]
fn multiline_paste_and_vertical_navigation_use_grapheme_columns() {
    let mut text = TextBuffer::new("", true);
    text.replace_selection("ab\r\n世界\rline");
    assert_eq!(text.text(), "ab\n世界\nline");
    text.move_to(1, false);
    text.vertical(1, false);
    assert_eq!(&text.text()[text.cursor()..], "界\nline");
    text.vertical(1, false);
    assert_eq!(&text.text()[text.cursor()..], "ine");
    text.line_edge(false, true, false);
    text.vertical(-99, false);
    assert_eq!(text.cursor(), 0);
    text.line_edge(true, true, true);
    assert_eq!(text.selected_text(), "ab\n世界\nline");
}

#[test]
fn single_line_paste_normalizes_newlines_and_new_edit_discards_redo() {
    let mut text = TextBuffer::new("", false);
    text.replace_selection("Project\r\nTwo");
    assert_eq!(text.text(), "Project Two");
    text.undo();
    text.replace_selection("Other");
    text.redo();
    assert_eq!(text.text(), "Other");
}

#[test]
fn read_only_report_can_be_selected_but_never_changed_by_any_edit_path() {
    let original = "Synthetic report\n".repeat(10_000);
    let mut report = TextBuffer::read_only(original.clone());
    report.select_all();
    assert_eq!(report.selected_text(), original);
    report.replace_selection("bad");
    report.replace_utf16(Some(0..4), "bad");
    report.replace_marked(None, "bad", None);
    report.delete(false);
    report.undo();
    report.redo();
    assert_eq!(report.text(), original);
    report.line_edge(true, true, false);
    assert_eq!(report.cursor(), original.len());
}

#[test]
fn selected_offsets_never_split_graphemes_and_load_clears_edit_history() {
    let mut text = TextBuffer::new("e\u{301}🧪", true);
    text.select(1, 5);
    assert_eq!(text.selection(), 0..3);
    text.replace_selection("x");
    text.load("saved");
    text.undo();
    assert_eq!(text.text(), "saved");
    assert_eq!(text.cursor(), 0);
}
