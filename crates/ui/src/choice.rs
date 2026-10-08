use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle, RoundedRectangle, StrokeAlignment},
};
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};

use sporos_app::action::Action;

use crate::{
    best_fit_font,
    legend::{self, Hint},
    usable_width, ACCENT_COLOR, BACKGROUND_COLOR, BODY_FONTS, DIM_COLOR, HEADER_FONT,
    HORIZONTAL_MARGIN, TEXT_COLOR,
};

/// The keypad legend along the bottom. The same three as the menu's: this is a
/// list with a cursor on it, so it is driven like one.
pub(crate) const HINTS: [Hint; 3] = [
    Hint::new(&[Action::Up, Action::Down], "move"),
    Hint::new(&[Action::Select], "select"),
    Hint::new(&[Action::Back], "back"),
];

/// Gap between the top edge and the title.
const TITLE_MARGIN: i32 = 4;

/// Matching the menu's, so a list of boxes is a list of boxes wherever it shows.
const ROW_HEIGHT: i32 = 24;
const BOX_HEIGHT: u32 = 20;
const BOX_RADIUS: u32 = 3;
const BOX_PADDING: i32 = 6;

/// The most rows any caller draws. What [`tests`] checks the layout against.
#[cfg(test)]
const MAX_ROWS: usize = 2;

/// A short list under a title saying what is being picked, drawn like the
/// menu's entries because it behaves like them. Used by the screens that pick
/// one of a couple of things before a tool starts: the phrase length, and which
/// SD card tool.
///
/// `labels` must be ASCII, for the reason the menu's are — see the note on
/// [`crate::menu::label`].
pub(crate) fn show_choice_screen<D>(display: &mut D, title: &str, labels: &[&str], selected: usize)
where
    D: DrawTarget<Color = Rgb565>,
    D::Error: core::fmt::Debug,
{
    display.clear(BACKGROUND_COLOR).expect("clear failed");

    let bounds = display.bounding_box();
    let center = bounds.center();
    let bottom = bounds.size.height as i32;
    let usable_width = usable_width(&bounds);

    HEADER_FONT
        .render_aligned(
            title,
            Point::new(center.x, TITLE_MARGIN),
            VerticalPosition::Top,
            HorizontalAlignment::Center,
            FontColor::Transparent(TEXT_COLOR),
            display,
        )
        .expect("title render failed");

    // One face for every row, chosen from the longest, so the rows read as one
    // list rather than as different sizes of thing.
    let longest = labels
        .iter()
        .max_by_key(|label| label.len())
        .expect("there is always a choice");
    let font = best_fit_font(&BODY_FONTS, *longest, usable_width);
    let left = HORIZONTAL_MARGIN as i32;

    let rows = labels.len() as i32;
    let first_row = center.y - (rows - 1) * ROW_HEIGHT / 2;

    for (index, label) in labels.iter().enumerate() {
        let is_selected = index == selected;
        let y = first_row + index as i32 * ROW_HEIGHT;

        let outline = Rectangle::new(
            Point::new(left, y - BOX_HEIGHT as i32 / 2),
            Size::new(usable_width, BOX_HEIGHT),
        );

        let style = if is_selected {
            PrimitiveStyleBuilder::new()
                .fill_color(ACCENT_COLOR)
                .build()
        } else {
            PrimitiveStyleBuilder::new()
                .stroke_color(DIM_COLOR)
                .stroke_width(1)
                .stroke_alignment(StrokeAlignment::Inside)
                .build()
        };

        RoundedRectangle::with_equal_corners(outline, Size::new(BOX_RADIUS, BOX_RADIUS))
            .into_styled(style)
            .draw(display)
            .expect("choice box render failed");

        // Inverted rather than tinted, as the menu's selection is.
        let color = if is_selected {
            BACKGROUND_COLOR
        } else {
            TEXT_COLOR
        };

        font.render(
            *label,
            Point::new(left + BOX_PADDING, y),
            VerticalPosition::Center,
            FontColor::Transparent(color),
            display,
        )
        .expect("label render failed");
    }

    let legend = legend::compose(&HINTS);
    best_fit_font(&BODY_FONTS, legend.as_str(), usable_width)
        .render_aligned(
            legend.as_str(),
            Point::new(center.x, bottom - 6),
            VerticalPosition::Bottom,
            HorizontalAlignment::Center,
            FontColor::Transparent(ACCENT_COLOR),
            display,
        )
        .expect("hint render failed");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same growing-towards-each-other problem the menu has, checked the
    /// same way. Two rows leave room to spare, which is the point of checking:
    /// it says so rather than leaving it to be assumed.
    #[test]
    fn the_rows_stay_clear_of_the_hint_line() {
        const PANEL: Size = Size::new(240, 135);

        let bounds = Rectangle::new(Point::zero(), PANEL);
        let usable = usable_width(&bounds);
        let bottom = PANEL.height as i32;

        let rows = MAX_ROWS as i32;
        let first_row = bounds.center().y - (rows - 1) * ROW_HEIGHT / 2;
        let last_row = first_row + (rows - 1) * ROW_HEIGHT;

        let boxes_bottom = last_row + BOX_HEIGHT as i32 / 2;
        let hint_top = bottom - 6 - text_height(legend::compose(&HINTS).as_str(), usable);

        assert!(
            boxes_bottom <= hint_top,
            "{rows} boxes reach y {boxes_bottom}, into the hint line at y {hint_top}",
        );
    }

    /// Checked against the longest label any caller passes.
    #[test]
    fn a_box_is_taller_than_the_label_in_it() {
        const PANEL: Size = Size::new(240, 135);

        let usable = usable_width(&Rectangle::new(Point::zero(), PANEL));

        for label in crate::length::LABELS.iter().chain(crate::sd::LABELS.iter()) {
            let height = text_height(label, usable);

            assert!(
                height < BOX_HEIGHT as i32,
                "a {height}px label does not fit a {BOX_HEIGHT}px box",
            );
        }
    }

    #[test]
    fn no_caller_draws_more_rows_than_were_checked() {
        assert!(crate::length::LABELS.len() <= MAX_ROWS);
        assert!(crate::sd::LABELS.len() <= MAX_ROWS);
    }

    fn text_height(text: &str, usable: u32) -> i32 {
        best_fit_font(&BODY_FONTS, text, usable)
            .get_rendered_dimensions(text, Point::zero(), VerticalPosition::Center)
            .expect("measure failed")
            .bounding_box
            .map(|box_| box_.size.height as i32)
            .unwrap_or_default()
    }
}
