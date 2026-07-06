use gpui::{Rgba, rgb};

pub(crate) mod font {
    pub(crate) const UI: &str = ".SystemUIFont";
}

pub(crate) mod opacity {
    pub(crate) const DEEMPHASIZED_ITEM: f32 = 0.72;
    pub(crate) const DEEMPHASIZED_ITEM_HOVER: f32 = 0.9;
}

pub(crate) mod color {
    use gpui::{Rgba, rgb};

    pub(crate) fn app_background() -> Rgba {
        rgb(0x0f1115)
    }

    pub(crate) fn panel_background() -> Rgba {
        rgb(0x14181d)
    }

    pub(crate) fn content_background() -> Rgba {
        rgb(0x0b0e12)
    }

    pub(crate) fn elevated_background() -> Rgba {
        rgb(0x171b20)
    }

    pub(crate) fn input_background() -> Rgba {
        rgb(0x0b1118)
    }

    pub(crate) fn border() -> Rgba {
        rgb(0x20262d)
    }

    pub(crate) fn border_subtle() -> Rgba {
        rgb(0x1a2027)
    }

    pub(crate) fn border_strong() -> Rgba {
        rgb(0x303946)
    }

    pub(crate) fn row_hover() -> Rgba {
        rgb(0x1b222b)
    }

    pub(crate) fn row_selected() -> Rgba {
        rgb(0x1d2733)
    }

    pub(crate) fn row_selected_active() -> Rgba {
        rgb(0x202b38)
    }

    pub(crate) fn row_selected_subtle() -> Rgba {
        rgb(0x19222c)
    }

    pub(crate) fn text_primary() -> Rgba {
        rgb(0xe6edf3)
    }

    pub(crate) fn text_secondary() -> Rgba {
        rgb(0xa8b3c1)
    }

    pub(crate) fn text_muted() -> Rgba {
        rgb(0x7f8997)
    }

    pub(crate) fn text_disabled() -> Rgba {
        rgb(0x586474)
    }

    pub(crate) fn accent() -> Rgba {
        rgb(0x8ab4f8)
    }

    pub(crate) fn accent_hover() -> Rgba {
        rgb(0xb4d3ff)
    }

    pub(crate) fn success() -> Rgba {
        rgb(0x34d399)
    }

    pub(crate) fn warning() -> Rgba {
        rgb(0xfbbf24)
    }

    pub(crate) fn danger() -> Rgba {
        rgb(0xf87171)
    }

    pub(crate) fn success_background() -> Rgba {
        rgb(0x0f2118)
    }

    pub(crate) fn warning_background() -> Rgba {
        rgb(0x211a0e)
    }

    pub(crate) fn danger_background() -> Rgba {
        rgb(0x241316)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Tone {
    Neutral,
    Info,
    Success,
    Warning,
    Danger,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ToneColors {
    pub(crate) text: Rgba,
    pub(crate) border: Rgba,
    pub(crate) background: Rgba,
}

pub(crate) fn tone_colors(tone: Tone) -> ToneColors {
    match tone {
        Tone::Neutral => ToneColors {
            text: color::text_muted(),
            border: color::border(),
            background: color::content_background(),
        },
        Tone::Info => ToneColors {
            text: color::accent(),
            border: rgb(0x2b4a70),
            background: rgb(0x101b2a),
        },
        Tone::Success => ToneColors {
            text: color::success(),
            border: rgb(0x23543a),
            background: color::success_background(),
        },
        Tone::Warning => ToneColors {
            text: color::warning(),
            border: rgb(0x6f4a13),
            background: color::warning_background(),
        },
        Tone::Danger => ToneColors {
            text: color::danger(),
            border: rgb(0x693238),
            background: color::danger_background(),
        },
    }
}

pub(crate) fn tone_text(tone: Tone) -> Rgba {
    tone_colors(tone).text
}
