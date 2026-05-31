use gpui::{Rgba, rgb};

use crate::diff::DiffLineKind;
use crate::visual::color;

pub(super) struct DiffLineStyleInput {
    pub(super) kind: DiffLineKind,
    pub(super) dragging_for_comment: bool,
    pub(super) selected_for_comment: bool,
    pub(super) has_thread_anchor: bool,
    pub(super) has_thread_range: bool,
    pub(super) has_syntax_highlights: bool,
}

pub(super) struct DiffLineStyle {
    pub(super) prefix: &'static str,
    pub(super) background: Rgba,
    pub(super) hover_background: Rgba,
    pub(super) text_color: Rgba,
    pub(super) code_text_color: Rgba,
}

pub(super) fn diff_line_style(input: DiffLineStyleInput) -> DiffLineStyle {
    let DiffLineStyleInput {
        kind,
        dragging_for_comment,
        selected_for_comment,
        has_thread_anchor,
        has_thread_range,
        has_syntax_highlights,
    } = input;
    let base = base_line_style(kind);
    let background = if dragging_for_comment {
        dragging_background(kind)
    } else if selected_for_comment {
        selected_background(kind)
    } else if has_thread_anchor {
        thread_anchor_background(kind)
    } else if has_thread_range {
        thread_range_background(kind)
    } else {
        base.background
    };
    let hover_background = if dragging_for_comment {
        dragging_hover_background(kind)
    } else if selected_for_comment {
        selected_hover_background(kind)
    } else if has_thread_anchor {
        rgb(0x2f2716)
    } else if has_thread_range {
        thread_range_hover_background(kind)
    } else {
        color::row_hover()
    };
    let code_text_color = if has_syntax_highlights {
        color::text_primary()
    } else {
        base.text_color
    };

    DiffLineStyle {
        prefix: base.prefix,
        background,
        hover_background,
        text_color: base.text_color,
        code_text_color,
    }
}

struct BaseDiffLineStyle {
    prefix: &'static str,
    background: Rgba,
    text_color: Rgba,
}

fn base_line_style(kind: DiffLineKind) -> BaseDiffLineStyle {
    match kind {
        DiffLineKind::Context => BaseDiffLineStyle {
            prefix: " ",
            background: color::content_background(),
            text_color: color::text_secondary(),
        },
        DiffLineKind::Added => BaseDiffLineStyle {
            prefix: "+",
            background: rgb(0x0d2118),
            text_color: rgb(0xa7f3d0),
        },
        DiffLineKind::Removed => BaseDiffLineStyle {
            prefix: "-",
            background: rgb(0x241316),
            text_color: rgb(0xfca5a5),
        },
        DiffLineKind::Metadata => BaseDiffLineStyle {
            prefix: "\\",
            background: rgb(0x11161d),
            text_color: color::text_muted(),
        },
    }
}

fn selected_background(kind: DiffLineKind) -> Rgba {
    match kind {
        DiffLineKind::Context | DiffLineKind::Metadata => rgb(0x1d2b3d),
        DiffLineKind::Added => rgb(0x143d2a),
        DiffLineKind::Removed => rgb(0x3e252b),
    }
}

fn dragging_background(kind: DiffLineKind) -> Rgba {
    match kind {
        DiffLineKind::Context | DiffLineKind::Metadata => rgb(0x26384e),
        DiffLineKind::Added => rgb(0x185037),
        DiffLineKind::Removed => rgb(0x56313a),
    }
}

fn thread_range_background(kind: DiffLineKind) -> Rgba {
    match kind {
        DiffLineKind::Context | DiffLineKind::Metadata => rgb(0x121922),
        DiffLineKind::Added => rgb(0x12281d),
        DiffLineKind::Removed => rgb(0x2b1b1f),
    }
}

fn thread_anchor_background(kind: DiffLineKind) -> Rgba {
    match kind {
        DiffLineKind::Context | DiffLineKind::Metadata => rgb(0x221e12),
        DiffLineKind::Added => rgb(0x202d18),
        DiffLineKind::Removed => rgb(0x31201b),
    }
}

fn dragging_hover_background(kind: DiffLineKind) -> Rgba {
    match kind {
        DiffLineKind::Added => rgb(0x20694a),
        DiffLineKind::Removed => rgb(0x704049),
        DiffLineKind::Context | DiffLineKind::Metadata => rgb(0x2a415d),
    }
}

fn selected_hover_background(kind: DiffLineKind) -> Rgba {
    match kind {
        DiffLineKind::Added => rgb(0x194b35),
        DiffLineKind::Removed => rgb(0x4c2e35),
        DiffLineKind::Context | DiffLineKind::Metadata => rgb(0x22344b),
    }
}

fn thread_range_hover_background(kind: DiffLineKind) -> Rgba {
    match kind {
        DiffLineKind::Added => rgb(0x193326),
        DiffLineKind::Removed => rgb(0x3a2327),
        DiffLineKind::Context | DiffLineKind::Metadata => rgb(0x17212b),
    }
}

#[cfg(test)]
mod tests {
    use gpui::rgb;

    use super::{DiffLineStyleInput, diff_line_style};
    use crate::diff::DiffLineKind;

    #[test]
    fn dragging_background_takes_precedence_over_selection_and_threads() {
        let style = diff_line_style(DiffLineStyleInput {
            kind: DiffLineKind::Added,
            dragging_for_comment: true,
            selected_for_comment: true,
            has_thread_anchor: true,
            has_thread_range: true,
            has_syntax_highlights: false,
        });

        assert_eq!(style.background, rgb(0x185037));
        assert_eq!(style.hover_background, rgb(0x20694a));
    }

    #[test]
    fn syntax_highlights_use_primary_code_text_color() {
        let style = diff_line_style(DiffLineStyleInput {
            kind: DiffLineKind::Removed,
            dragging_for_comment: false,
            selected_for_comment: false,
            has_thread_anchor: false,
            has_thread_range: false,
            has_syntax_highlights: true,
        });

        assert_eq!(style.prefix, "-");
        assert_eq!(style.code_text_color, crate::visual::color::text_primary());
    }
}
