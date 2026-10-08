use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use sporos_app::action::Action;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};

use crate::{
    best_fit_font,
    legend::{self, Hint},
    usable_width, ACCENT_COLOR, BACKGROUND_COLOR, BODY_FONTS, TEXT_COLOR, WARNING_COLOR,
};

/// The legend under a finished message.
pub(crate) const HINTS: [Hint; 1] = [Hint::new(&[Action::Back], "back")];

/// Distance from the bottom edge to the bottom of the legend, as elsewhere.
const HINT_BOTTOM: i32 = 6;

/// `text` in the middle of the panel, in the warning colour when something the
/// user asked for did not happen.
///
/// `busy` is the wait on the card, when no key does anything, so it has no
/// legend. The firmware answers before the next key is read, so it is rarely
/// on screen long enough to see.
pub(crate) fn show_message_screen<D>(display: &mut D, text: &str, warning: bool, busy: bool)
where
    D: DrawTarget<Color = Rgb565>,
    D::Error: core::fmt::Debug,
{
    display.clear(BACKGROUND_COLOR).expect("clear failed");

    let bounds = display.bounding_box();
    let center = bounds.center();
    let bottom = bounds.size.height as i32;
    let usable_width = usable_width(&bounds);

    let color = if warning { WARNING_COLOR } else { TEXT_COLOR };

    best_fit_font(&BODY_FONTS, text, usable_width)
        .render_aligned(
            text,
            center,
            VerticalPosition::Center,
            HorizontalAlignment::Center,
            FontColor::Transparent(color),
            display,
        )
        .expect("message render failed");

    if busy {
        return;
    }

    let legend = legend::compose(hints(busy));
    best_fit_font(&BODY_FONTS, legend.as_str(), usable_width)
        .render_aligned(
            legend.as_str(),
            Point::new(center.x, bottom - HINT_BOTTOM),
            VerticalPosition::Bottom,
            HorizontalAlignment::Center,
            FontColor::Transparent(ACCENT_COLOR),
            display,
        )
        .expect("hint render failed");
}

/// The legend for a message: none while the card is busy.
pub(crate) fn hints(busy: bool) -> &'static [Hint] {
    if busy {
        &[]
    } else {
        &HINTS
    }
}
