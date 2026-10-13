use embedded_graphics::{pixelcolor::Rgb565, prelude::*};

use sporos_app::sd::SdTool;

use crate::choice;

const TITLE_TEXT: &str = "SD card";

/// What each tool is called on the panel, in [`SdTool::ALL`] order.
pub(crate) const LABELS: [&str; SdTool::ALL.len()] = ["Store seed", "Read seed"];

/// Store or read, picked the way a phrase length is.
pub(crate) fn show_sd_screen<D>(display: &mut D, selected: SdTool)
where
    D: DrawTarget<Color = Rgb565>,
    D::Error: core::fmt::Debug,
{
    let index = SdTool::ALL
        .iter()
        .position(|tool| *tool == selected)
        .expect("every tool is in ALL");

    choice::show_choice_screen(display, TITLE_TEXT, &LABELS, index);
}
