//! Custom native input using GPUI's EntityInputHandler and our tested TextBuffer.
//! GPUI's upstream input example documents the handler/element integration;
//! GPUI is Apache-2.0 licensed. Editing and IME transactions live in editor.rs.
use crate::editor::TextBuffer;
use gpui::{
    App, AvailableSpace, Bounds, ClipboardItem, Context, CursorStyle, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId,
    KeyBinding, LayoutId, MouseButton, Pixels, Point, ScrollHandle, SharedString, Style, TextAlign,
    TextRun, UTF16Selection, UnderlineStyle, Window, WrappedLine, actions, div, fill, point,
    prelude::*, px, relative, rgb, rgba, size,
};
use std::{cell::RefCell, ops::Range, rc::Rc};

actions!(
    pfl_input,
    [
        Backspace,
        ForwardDelete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        Up,
        Down,
        SelectUp,
        SelectDown,
        Home,
        End,
        SelectHome,
        SelectEnd,
        DocumentHome,
        DocumentEnd,
        SelectDocumentHome,
        SelectDocumentEnd,
        SelectAll,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        Newline,
        PageUp,
        PageDown,
    ]
);

pub fn bind_keys(cx: &mut App) {
    let mut keys = vec![
        KeyBinding::new("backspace", Backspace, Some("PflText")),
        KeyBinding::new("delete", ForwardDelete, Some("PflText")),
        KeyBinding::new("left", Left, Some("PflText")),
        KeyBinding::new("right", Right, Some("PflText")),
        KeyBinding::new("shift-left", SelectLeft, Some("PflText")),
        KeyBinding::new("shift-right", SelectRight, Some("PflText")),
        KeyBinding::new("up", Up, Some("PflText")),
        KeyBinding::new("down", Down, Some("PflText")),
        KeyBinding::new("shift-up", SelectUp, Some("PflText")),
        KeyBinding::new("shift-down", SelectDown, Some("PflText")),
        KeyBinding::new("home", Home, Some("PflText")),
        KeyBinding::new("end", End, Some("PflText")),
        KeyBinding::new("shift-home", SelectHome, Some("PflText")),
        KeyBinding::new("shift-end", SelectEnd, Some("PflText")),
        KeyBinding::new("enter", Newline, Some("PflText")),
        KeyBinding::new("pageup", PageUp, Some("PflText")),
        KeyBinding::new("pagedown", PageDown, Some("PflText")),
    ];
    macro_rules! platform_keys {
        ($modifier:literal) => {
            keys.extend([
                KeyBinding::new(concat!($modifier, "-a"), SelectAll, Some("PflText")),
                KeyBinding::new(concat!($modifier, "-c"), Copy, Some("PflText")),
                KeyBinding::new(concat!($modifier, "-x"), Cut, Some("PflText")),
                KeyBinding::new(concat!($modifier, "-v"), Paste, Some("PflText")),
                KeyBinding::new(concat!($modifier, "-z"), Undo, Some("PflText")),
                KeyBinding::new(concat!($modifier, "-shift-z"), Redo, Some("PflText")),
                KeyBinding::new(concat!($modifier, "-home"), DocumentHome, Some("PflText")),
                KeyBinding::new(concat!($modifier, "-end"), DocumentEnd, Some("PflText")),
                KeyBinding::new(
                    concat!($modifier, "-shift-home"),
                    SelectDocumentHome,
                    Some("PflText"),
                ),
                KeyBinding::new(
                    concat!($modifier, "-shift-end"),
                    SelectDocumentEnd,
                    Some("PflText"),
                ),
            ]);
        };
    }
    #[cfg(target_os = "macos")]
    platform_keys!("cmd");
    #[cfg(not(target_os = "macos"))]
    platform_keys!("ctrl");
    #[cfg(not(target_os = "macos"))]
    keys.push(KeyBinding::new("ctrl-y", Redo, Some("PflText")));
    #[cfg(target_os = "macos")]
    keys.extend([
        KeyBinding::new("cmd-left", Home, Some("PflText")),
        KeyBinding::new("cmd-right", End, Some("PflText")),
        KeyBinding::new("cmd-up", DocumentHome, Some("PflText")),
        KeyBinding::new("cmd-down", DocumentEnd, Some("PflText")),
    ]);
    cx.bind_keys(keys);
}

struct Row {
    start: usize,
    top: Pixels,
    line: WrappedLine,
}
struct TextLayout {
    text: String,
    rows: Vec<Row>,
}

impl TextLayout {
    fn position(&self, index: usize) -> Point<Pixels> {
        let row = self.rows.iter().rev().find(|r| r.start <= index);
        row.and_then(|r| {
            r.line
                .position_for_index((index - r.start).min(r.line.len()), px(22.))
                .map(|p| point(p.x, p.y + r.top))
        })
        .unwrap_or_default()
    }

    fn index(&self, position: Point<Pixels>) -> usize {
        let Some(row) = self
            .rows
            .iter()
            .rev()
            .find(|r| r.top <= position.y)
            .or_else(|| self.rows.first())
        else {
            return 0;
        };
        let offset = row
            .line
            .closest_index_for_position(
                point(position.x, (position.y - row.top).max(px(0.))),
                px(22.),
            )
            .unwrap_or_else(|offset| offset);
        (row.start + offset).min(self.text.len())
    }
}

pub struct TextInput {
    pub buffer: TextBuffer,
    focus: FocusHandle,
    height: Pixels,
    selecting: bool,
    last_layout: Option<Rc<TextLayout>>,
    last_bounds: Option<Bounds<Pixels>>,
    scroll: ScrollHandle,
    reveal: bool,
}

impl TextInput {
    pub fn new(buffer: TextBuffer, height: f32, cx: &mut Context<Self>) -> Self {
        Self {
            buffer,
            focus: cx.focus_handle().tab_stop(true),
            height: px(height),
            selecting: false,
            last_layout: None,
            last_bounds: None,
            scroll: ScrollHandle::new(),
            reveal: false,
        }
    }

    pub fn load(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.buffer.load(text);
        self.last_layout = None;
        self.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }

    fn changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reveal = true;
        window.notify_text_input_changed(&self.focus);
        window.notify_text_selection_changed(&self.focus);
        cx.notify();
    }

    fn mouse_index(&self, position: Point<Pixels>) -> usize {
        match (&self.last_layout, self.last_bounds) {
            (Some(layout), Some(bounds)) => layout.index(position - bounds.origin),
            _ => 0,
        }
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.buffer.from_utf16(range);
        *actual = Some(self.buffer.to_utf16(range.clone()));
        Some(self.buffer.text()[range].to_owned())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.buffer.to_utf16(self.buffer.selection()),
            reversed: self.buffer.reversed(),
        })
    }
    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.buffer.from_utf16(range);
        self.buffer.select(range.start, range.end);
        self.changed(window, cx);
    }
    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.buffer.text().encode_utf16().count())
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.buffer
            .marked()
            .map(|range| self.buffer.to_utf16(range))
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.buffer.unmark();
        self.changed(window, cx);
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.buffer.replace_utf16(range, text);
        self.changed(window, cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.buffer.replace_marked(range, text, selected);
        self.changed(window, cx);
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.last_layout.as_ref()?;
        if layout.text != self.buffer.text() {
            return None;
        }
        let range = self.buffer.from_utf16(range);
        let start = layout.position(range.start) + self.last_bounds?.origin;
        Some(Bounds::new(start, size(px(2.), px(22.))))
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let index = self.mouse_index(point);
        Some(self.buffer.to_utf16(index..index).start)
    }
}

impl Render for TextInput {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("text-input")
            .key_context("PflText")
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .w_full()
            .h(self.height)
            .border_1()
            .rounded_md()
            .border_color(rgb(0x3d5268))
            .bg(rgb(0x101b29))
            .text_color(rgb(0xe3eaf3))
            .text_size(px(14.))
            .line_height(px(22.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    this.focus.focus(window, cx);
                    let index = this.mouse_index(event.position);
                    this.buffer.move_to(index, event.modifiers.shift);
                    this.selecting = true;
                    this.changed(window, cx);
                }),
            )
            .on_mouse_move(
                cx.listener(|this, event: &gpui::MouseMoveEvent, window, cx| {
                    if this.selecting {
                        this.buffer.move_to(this.mouse_index(event.position), true);
                        this.changed(window, cx);
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .on_action(cx.listener(|this, _: &Backspace, w, cx| {
                this.buffer.delete(false);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &ForwardDelete, w, cx| {
                this.buffer.delete(true);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &Left, w, cx| {
                this.buffer.horizontal(false, false);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &Right, w, cx| {
                this.buffer.horizontal(true, false);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectLeft, w, cx| {
                this.buffer.horizontal(false, true);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectRight, w, cx| {
                this.buffer.horizontal(true, true);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &Up, w, cx| {
                this.buffer.vertical(-1, false);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &Down, w, cx| {
                this.buffer.vertical(1, false);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectUp, w, cx| {
                this.buffer.vertical(-1, true);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectDown, w, cx| {
                this.buffer.vertical(1, true);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &Home, w, cx| {
                this.buffer.line_edge(false, false, false);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &End, w, cx| {
                this.buffer.line_edge(true, false, false);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectHome, w, cx| {
                this.buffer.line_edge(false, false, true);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectEnd, w, cx| {
                this.buffer.line_edge(true, false, true);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &DocumentHome, w, cx| {
                this.buffer.line_edge(false, true, false);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &DocumentEnd, w, cx| {
                this.buffer.line_edge(true, true, false);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectDocumentHome, w, cx| {
                this.buffer.line_edge(false, true, true);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectDocumentEnd, w, cx| {
                this.buffer.line_edge(true, true, true);
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectAll, w, cx| {
                this.buffer.select_all();
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &Undo, w, cx| {
                this.buffer.undo();
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &Redo, w, cx| {
                this.buffer.redo();
                this.changed(w, cx);
            }))
            .on_action(cx.listener(|this, _: &Newline, w, cx| {
                if this.buffer.multiline() {
                    this.buffer.replace_selection("\n");
                    this.changed(w, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                if !this.buffer.selection().is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        this.buffer.selected_text().to_owned(),
                    ));
                }
            }))
            .on_action(cx.listener(|this, _: &Cut, w, cx| {
                if !this.buffer.selection().is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        this.buffer.selected_text().to_owned(),
                    ));
                    this.buffer.replace_selection("");
                    this.changed(w, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Paste, w, cx| {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    this.buffer.replace_selection(&text);
                    this.changed(w, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &PageUp, _, cx| {
                let offset = this.scroll.offset();
                this.scroll
                    .set_offset(point(offset.x, offset.y + this.height * 0.85));
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &PageDown, _, cx| {
                let offset = this.scroll.offset();
                this.scroll
                    .set_offset(point(offset.x, offset.y - this.height * 0.85));
                cx.notify();
            }))
            .child(div().p_2().child(InputElement {
                input: cx.entity(),
                layout: Rc::new(RefCell::new(None)),
            }))
    }
}

struct InputElement {
    input: Entity<TextInput>,
    layout: Rc<RefCell<Option<Rc<TextLayout>>>>,
}

impl Element for InputElement {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let buffer = self.input.read(cx).buffer.clone();
        let style = window.text_style();
        let layout = self.layout.clone();
        let mut element_style = Style::default();
        element_style.size.width = relative(1.).into();
        let id =
            window.request_measured_layout(element_style, move |known, available, window, _| {
                let width = known
                    .width
                    .or(match available.width {
                        AvailableSpace::Definite(width) => Some(width),
                        _ => None,
                    })
                    .unwrap_or(px(600.))
                    .max(px(20.));
                let text = buffer.text().to_owned();
                let selection = buffer.selection();
                let marked = buffer.marked();
                let mut stops = vec![0, text.len(), selection.start, selection.end];
                if let Some(marked) = &marked {
                    stops.extend([marked.start, marked.end]);
                }
                stops.sort_unstable();
                stops.dedup();
                let runs: Vec<_> = stops
                    .windows(2)
                    .filter(|r| r[1] > r[0])
                    .map(|r| TextRun {
                        len: r[1] - r[0],
                        font: style.font(),
                        color: style.color,
                        background_color: (r[0] >= selection.start && r[1] <= selection.end)
                            .then_some(rgba(0x368ad955).into()),
                        underline: marked
                            .as_ref()
                            .filter(|m| r[0] >= m.start && r[1] <= m.end)
                            .map(|_| UnderlineStyle {
                                color: Some(style.color),
                                thickness: px(1.),
                                wavy: false,
                            }),
                        strikethrough: None,
                    })
                    .collect();
                let shaped = window
                    .text_system()
                    .shape_text(
                        SharedString::from(text.clone()),
                        px(14.),
                        &runs,
                        Some(width),
                        None,
                    )
                    .unwrap_or_default();
                let mut rows = Vec::new();
                let mut start = 0;
                let mut height = px(0.);
                for line in shaped {
                    let line_height = line.size(px(22.)).height.max(px(22.));
                    let len = line.len();
                    rows.push(Row {
                        start,
                        top: height,
                        line,
                    });
                    height += line_height;
                    start += len + 1;
                }
                height = height.max(px(22.));
                *layout.borrow_mut() = Some(Rc::new(TextLayout { text, rows }));
                size(width, height)
            });
        (id, ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut Window,
        _: &mut App,
    ) {
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(layout) = self.layout.borrow().clone() else {
            return;
        };
        let input = self.input.read(cx);
        let focus = input.focus.clone();
        let cursor = input.buffer.cursor();
        let draw_cursor = input.buffer.selection().is_empty()
            && focus.is_focused(window)
            && !input.buffer.is_read_only();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        for row in &layout.rows {
            let origin = bounds.origin + point(px(0.), row.top);
            let _ = row.line.paint_background(
                origin,
                px(22.),
                TextAlign::Left,
                Some(bounds),
                window,
                cx,
            );
            let _ = row
                .line
                .paint(origin, px(22.), TextAlign::Left, Some(bounds), window, cx);
        }
        if draw_cursor {
            let position = bounds.origin + layout.position(cursor);
            window.paint_quad(fill(
                Bounds::new(position, size(px(1.5), px(22.))),
                rgb(0x8dc6ff),
            ));
        }
        self.input.update(cx, |input, cx| {
            if input.reveal {
                let position = layout.position(cursor);
                let offset = input.scroll.offset();
                let top = -offset.y;
                let bottom = top + input.height - px(28.);
                if position.y < top {
                    input.scroll.set_offset(point(offset.x, -position.y));
                } else if position.y + px(22.) > bottom {
                    input
                        .scroll
                        .set_offset(point(offset.x, -(position.y + px(50.) - input.height)));
                }
                input.reveal = false;
                cx.notify();
            }
            input.last_layout = Some(layout);
            input.last_bounds = Some(bounds);
        });
    }
}
