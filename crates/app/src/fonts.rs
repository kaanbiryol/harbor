use gpui::{App, Result};
use std::borrow::Cow;

const IBM_PLEX_SANS_REGULAR: &[u8] =
    include_bytes!("../../ui/assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf");
const IBM_PLEX_SANS_ITALIC: &[u8] =
    include_bytes!("../../ui/assets/fonts/ibm-plex-sans/IBMPlexSans-Italic.ttf");
const IBM_PLEX_SANS_SEMIBOLD: &[u8] =
    include_bytes!("../../ui/assets/fonts/ibm-plex-sans/IBMPlexSans-SemiBold.ttf");
const IBM_PLEX_SANS_SEMIBOLD_ITALIC: &[u8] =
    include_bytes!("../../ui/assets/fonts/ibm-plex-sans/IBMPlexSans-SemiBoldItalic.ttf");
const LILEX_REGULAR: &[u8] = include_bytes!("../../ui/assets/fonts/lilex/Lilex-Regular.ttf");
const LILEX_ITALIC: &[u8] = include_bytes!("../../ui/assets/fonts/lilex/Lilex-Italic.ttf");
const LILEX_SEMIBOLD: &[u8] = include_bytes!("../../ui/assets/fonts/lilex/Lilex-SemiBold.ttf");
const LILEX_SEMIBOLD_ITALIC: &[u8] =
    include_bytes!("../../ui/assets/fonts/lilex/Lilex-SemiBoldItalic.ttf");

pub(crate) fn install(cx: &mut App) -> Result<()> {
    cx.text_system().add_fonts(vec![
        Cow::Borrowed(IBM_PLEX_SANS_REGULAR),
        Cow::Borrowed(IBM_PLEX_SANS_ITALIC),
        Cow::Borrowed(IBM_PLEX_SANS_SEMIBOLD),
        Cow::Borrowed(IBM_PLEX_SANS_SEMIBOLD_ITALIC),
        Cow::Borrowed(LILEX_REGULAR),
        Cow::Borrowed(LILEX_ITALIC),
        Cow::Borrowed(LILEX_SEMIBOLD),
        Cow::Borrowed(LILEX_SEMIBOLD_ITALIC),
    ])
}
