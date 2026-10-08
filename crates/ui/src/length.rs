use embedded_graphics::{pixelcolor::Rgb565, prelude::*};

use sporos_core::bip39::SeedLength;

use crate::choice;

/// What the question is, above the choices.
const TITLE_TEXT: &str = "phrase length";

/// What each length is called on the panel, in [`SeedLength::ALL`] order.
pub(crate) const LABELS: [&str; SeedLength::ALL.len()] = ["12 words", "24 words"];

/// The two lengths stacked down the middle, drawn like menu entries because they
/// behave like them.
pub(crate) fn show_length_screen<D>(display: &mut D, selected: SeedLength)
where
    D: DrawTarget<Color = Rgb565>,
    D::Error: core::fmt::Debug,
{
    let index = SeedLength::ALL
        .iter()
        .position(|length| *length == selected)
        .expect("every length is in ALL");

    choice::show_choice_screen(display, TITLE_TEXT, &LABELS, index);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_length_has_its_own_label() {
        for (length, label) in SeedLength::ALL.iter().zip(LABELS) {
            assert!(
                label.starts_with(&length.total_words().to_string()),
                "{length:?} is labelled {label:?}",
            );
        }
    }
}
