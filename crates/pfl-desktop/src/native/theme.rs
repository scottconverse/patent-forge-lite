//! Visual language for the native desktop, taken from the PatentForge identity
//! guide: Ink, Paper, Copper and Hot copper, with Barlow for all type.
//!
//! Everything here is presentation only: no state, no commands, no provider work.
use gpui::{BoxShadow, Div, FontWeight, SharedString, div, prelude::*, px, rgb, rgba};

// Brand primitives, exactly as specified.
pub const INK: u32 = 0x102b40;
pub const PAPER: u32 = 0xf5f1e8;
pub const COPPER: u32 = 0xb65a32;
pub const HOT_COPPER: u32 = 0xf08f56;

// Ink ladder: the branded base plus darker and lighter ink for elevation.
pub const BG: u32 = INK;
pub const RAIL: u32 = 0x0b1f2f;
pub const SURFACE: u32 = 0x16344b;
pub const SURFACE_2: u32 = 0x1d4059;
pub const BORDER: u32 = 0x2b4a61;
pub const BORDER_SOFT: u32 = 0x21405a;

// Paper ladder for text on ink.
pub const TEXT: u32 = PAPER;
pub const MUTED: u32 = 0xc3cbc9;
pub const FAINT: u32 = 0x8ea0a8;

pub const ACCENT: u32 = COPPER;
pub const ACCENT_DIM: u32 = 0x3a2418;
pub const OK: u32 = 0x7fb69a;
pub const WARN: u32 = 0xe0a458;
pub const DANGER: u32 = 0xd9614f;
pub const VIOLET: u32 = 0xa78bfa;

// Document surfaces: the guide says "use ink on paper for documents".
pub const DOC_BG: u32 = PAPER;
pub const DOC_TEXT: u32 = INK;
pub const DOC_MUTED: u32 = 0x4a6172;
pub const DOC_RULE: u32 = 0xd8cfbe;
pub const DOC_CODE: u32 = 0xeee8db;

#[allow(dead_code)]
pub const DOC_COPPER: u32 = COPPER;
pub fn card_shadow() -> Vec<BoxShadow> {
    vec![
        BoxShadow::new(px(0.), px(12.), rgba(0x00000059).into())
            .blur_radius(px(32.))
            .spread_radius(px(-12.)),
    ]
}

/// A bordered, elevated panel. The base surface for most content.
pub fn panel() -> Div {
    div()
        .bg(rgb(SURFACE))
        .rounded_lg()
        .border_1()
        .border_color(rgb(BORDER))
        .shadow(card_shadow())
}

/// Small uppercase-ish label used above values.
pub fn micro(label: impl Into<SharedString>) -> Div {
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(FAINT))
        .child(label.into())
}

/// Coloured status chip.
pub fn pill(label: impl Into<SharedString>, fg: u32, bg: u32) -> Div {
    div()
        .px_2()
        .py_1()
        .rounded_full()
        .bg(rgb(bg))
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(fg))
        .child(label.into())
}

pub fn dot(color: u32) -> Div {
    div().w(px(6.)).h(px(6.)).rounded_full().bg(rgb(color))
}

/// Segmented progress bar: one rounded segment per stage.
pub fn progress(done: usize, total: usize, color: u32) -> Div {
    let total = total.max(1);
    let mut row = div().flex().flex_row().items_center().gap_1().w_full();
    for index in 0..total {
        let filled = index < done;
        row = row.child(div().h(px(6.)).flex_1().rounded_full().bg(if filled {
            rgb(color)
        } else {
            rgb(SURFACE_2)
        }));
    }
    row
}

pub fn divider() -> Div {
    div().h(px(1.)).w_full().bg(rgb(BORDER_SOFT))
}

/// Outcome colour pairing for a run state.
pub fn outcome_colors(outcome: &str) -> (u32, u32) {
    match outcome.to_ascii_lowercase().as_str() {
        "completed" => (OK, 0x14322a),
        "running" => (HOT_COPPER, 0x3a2418),
        "cancelling" | "cancelled" => (WARN, 0x352a13),
        "failed" => (DANGER, 0x361a19),
        "interrupted" => (VIOLET, 0x2a2340),
        _ => (MUTED, 0x1b3247),
    }
}

/// A tiny geometric icon set drawn from GPUI primitives — no icon font or SVG
/// asset pipeline is required, and every glyph scales with the layout.
#[derive(Clone, Copy)]
pub enum Icon {
    Projects,
    Intake,
    Analysis,
    History,
    Report,
    Setup,
    Quit,
}

pub fn icon(kind: Icon, color: u32) -> Div {
    let stroke = rgb(color);
    let frame = div()
        .w(px(18.))
        .h(px(18.))
        .flex()
        .items_center()
        .justify_center();
    match kind {
        Icon::Projects => frame.child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_1()
                .w(px(16.))
                .child(div().w(px(6.)).h(px(6.)).rounded_sm().bg(stroke))
                .child(div().w(px(6.)).h(px(6.)).rounded_sm().bg(stroke))
                .child(div().w(px(6.)).h(px(6.)).rounded_sm().bg(stroke))
                .child(div().w(px(6.)).h(px(6.)).rounded_sm().bg(stroke)),
        ),
        Icon::Intake => frame.child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().w(px(15.)).h(px(2.)).rounded_full().bg(stroke))
                .child(div().w(px(11.)).h(px(2.)).rounded_full().bg(stroke))
                .child(div().w(px(13.)).h(px(2.)).rounded_full().bg(stroke)),
        ),
        Icon::Analysis => frame.child(
            div()
                .flex()
                .flex_row()
                .items_end()
                .gap_1()
                .h(px(14.))
                .child(div().w(px(3.)).h(px(7.)).rounded_sm().bg(stroke))
                .child(div().w(px(3.)).h(px(11.)).rounded_sm().bg(stroke))
                .child(div().w(px(3.)).h(px(14.)).rounded_sm().bg(stroke)),
        ),
        Icon::History => frame.child(
            div()
                .w(px(14.))
                .h(px(14.))
                .rounded_full()
                .border_1()
                .border_color(stroke)
                .flex()
                .items_center()
                .justify_center()
                .child(div().w(px(2.)).h(px(5.)).rounded_full().bg(stroke)),
        ),
        Icon::Report => frame.child(
            div()
                .w(px(12.))
                .h(px(15.))
                .rounded_sm()
                .border_1()
                .border_color(stroke)
                .flex()
                .flex_col()
                .justify_center()
                .items_center()
                .gap_1()
                .child(div().w(px(7.)).h(px(1.5)).rounded_full().bg(stroke))
                .child(div().w(px(7.)).h(px(1.5)).rounded_full().bg(stroke))
                .child(div().w(px(4.)).h(px(1.5)).rounded_full().bg(stroke)),
        ),
        Icon::Setup => frame.child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .w(px(15.))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(div().w(px(9.)).h(px(2.)).rounded_full().bg(stroke))
                        .child(div().w(px(4.)).h(px(4.)).rounded_full().bg(stroke)),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(div().w(px(4.)).h(px(4.)).rounded_full().bg(stroke))
                        .child(div().w(px(9.)).h(px(2.)).rounded_full().bg(stroke)),
                ),
        ),
        Icon::Quit => frame.child(
            div()
                .w(px(14.))
                .h(px(14.))
                .rounded_full()
                .border_1()
                .border_color(stroke)
                .flex()
                .items_center()
                .justify_center()
                .child(div().w(px(2.)).h(px(7.)).rounded_full().bg(stroke)),
        ),
    }
}

/// Human date from epoch milliseconds, without pulling a calendar crate.
pub fn fmt_date(ms: u64) -> String {
    let secs = ms / 1000;
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (hour, minute) = (rem / 3600, (rem % 3600) / 60);
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{y:04}-{m:02}-{d:02} {hour:02}:{minute:02}")
}
