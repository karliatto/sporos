//! The SD card tools: store a phrase on the card, or read back the one there.
//!
//! This crate touches no hardware, so the workflow cannot write the card
//! itself. It *asks*: while it is waiting on the card, [`Sd::request`] says what
//! for, the firmware does the I/O, and hands the outcome back through
//! [`Sd::stored`] or [`Sd::loaded`]. Everything around that — which screen
//! shows, what a key does, whether what came off the card is a phrase at all —
//! stays here, where it is tested.

use sporos_core::bip39::{self, Mnemonic, SeedLength};

use crate::{
    action::Action,
    seed_file,
    view::{self, View},
    word_entry::WordEntry,
    workflow::Outcome,
    xor::INVALID_TEXT,
};

/// The two tools behind the one menu entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SdTool {
    Store,
    Read,
}

impl SdTool {
    /// Every tool, in the order they are drawn.
    pub const ALL: [Self; 2] = [Self::Store, Self::Read];
}

/// What the workflow is waiting on the card for.
///
/// Deliberately not `Debug`: it can carry a phrase, and printing it while
/// debugging would put the phrase on the serial line.
pub enum SdRequest<'a> {
    /// Write this phrase to [`seed_file::SEED_FILE`], replacing what is there.
    Store(&'a Mnemonic),
    /// Read [`seed_file::SEED_FILE`].
    Load,
}

/// Why the card did not do what was asked, as far as the user needs to know.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SdError {
    /// No card answered: none inserted, or not seated.
    NoCard,
    /// The card is there, but the file is not.
    NotFound,
    /// Anything else the card or its filesystem refused.
    Failed,
}

const WRITING_TEXT: &str = "Writing to SD...";
const READING_TEXT: &str = "Reading SD...";
const STORED_TEXT: &str = "Seed stored on SD";
const NO_CARD_TEXT: &str = "No SD card";
const NOT_FOUND_TEXT: &str = "No seed on card";
const WRITE_FAILED_TEXT: &str = "SD write failed";
const READ_FAILED_TEXT: &str = "SD read failed";
const INVALID_SEED_TEXT: &str = "Invalid seed on card";

#[derive(Clone)]
pub(crate) struct Sd {
    step: Step,
}

/// Large for the same reason `Generate`'s is — room for a 24-word phrase, with
/// no allocator to box it into. See the note there.
#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
enum Step {
    Pick {
        selected: SdTool,
    },
    /// Store: 12 or 24, picked before anything is typed.
    Length {
        selected: SeedLength,
    },
    /// Store: the phrase, typed in full. `invalid` marks a complete phrase whose
    /// final word does not match the rest, as on the XOR tool.
    Words {
        length: SeedLength,
        entry: WordEntry,
        invalid: bool,
    },
    /// Store: waiting on the card. The phrase is held only until it is written.
    Writing {
        mnemonic: Mnemonic,
    },
    /// Read: waiting on the card.
    Reading,
    /// Read: what came off the card.
    Phrase {
        mnemonic: Mnemonic,
        page: usize,
    },
    /// Either tool, finished: what happened, until `Back`.
    Done {
        from: SdTool,
        text: &'static str,
        warning: bool,
    },
}

impl Sd {
    pub(crate) fn new() -> Self {
        Self {
            step: Step::Pick {
                selected: SdTool::Store,
            },
        }
    }

    pub(crate) fn press(&mut self, action: Action) -> Outcome {
        let changed = match &mut self.step {
            Step::Pick { selected } => match action {
                Action::Back => return Outcome::Exit,
                // Two entries, so either key is the other one.
                Action::Up | Action::Down => {
                    *selected = match *selected {
                        SdTool::Store => SdTool::Read,
                        SdTool::Read => SdTool::Store,
                    };

                    true
                }
                Action::Select => {
                    self.step = match *selected {
                        SdTool::Store => Step::Length {
                            selected: SeedLength::Words12,
                        },
                        SdTool::Read => Step::Reading,
                    };

                    true
                }
                _ => false,
            },
            Step::Length { selected } => match action {
                Action::Back => {
                    self.back_to_pick(SdTool::Store);

                    true
                }
                Action::Up | Action::Down => {
                    *selected = match *selected {
                        SeedLength::Words12 => SeedLength::Words24,
                        SeedLength::Words24 => SeedLength::Words12,
                    };

                    true
                }
                Action::Select => {
                    let length = *selected;
                    self.step = Step::Words {
                        length,
                        entry: WordEntry::full(length),
                        invalid: false,
                    };

                    true
                }
                _ => false,
            },
            Step::Words {
                length,
                entry,
                invalid,
            } => {
                // A refused phrase is complete, so the only way on is back to
                // its last word, where the mistake has to be corrected.
                if *invalid {
                    if action != Action::Back {
                        return Outcome::Unchanged;
                    }

                    *invalid = false;
                    entry.delete();

                    return Outcome::Redraw;
                }

                if action == Action::Back && entry.is_empty() {
                    self.step = Step::Length { selected: *length };

                    return Outcome::Redraw;
                }

                let changed = entry.press(action);

                if changed && entry.is_complete() {
                    match bip39::parse(*length, entry.accepted()) {
                        Some(mnemonic) => self.step = Step::Writing { mnemonic },
                        None => *invalid = true,
                    }
                }

                changed
            }
            // Nothing to press while the card is busy; the firmware answers
            // before the next key is read.
            Step::Writing { .. } | Step::Reading => false,
            Step::Phrase { mnemonic, page } => {
                let pages = view::phrase_pages(mnemonic);

                match action {
                    Action::Back => {
                        self.back_to_pick(SdTool::Read);

                        true
                    }
                    Action::Left | Action::Right if pages > 1 => {
                        *page = if action == Action::Right {
                            (*page + 1) % pages
                        } else {
                            (*page + pages - 1) % pages
                        };

                        true
                    }
                    _ => false,
                }
            }
            Step::Done { from, .. } => {
                if action == Action::Back {
                    let from = *from;
                    self.back_to_pick(from);
                }

                action == Action::Back
            }
        };

        if changed {
            Outcome::Redraw
        } else {
            Outcome::Unchanged
        }
    }

    /// What the workflow is waiting on the card for, if anything.
    pub(crate) fn request(&self) -> Option<SdRequest<'_>> {
        match &self.step {
            Step::Writing { mnemonic } => Some(SdRequest::Store(mnemonic)),
            Step::Reading => Some(SdRequest::Load),
            _ => None,
        }
    }

    /// The outcome of a [`SdRequest::Store`]. Ignored unless one is pending.
    pub(crate) fn stored(&mut self, result: Result<(), SdError>) {
        if !matches!(self.step, Step::Writing { .. }) {
            return;
        }

        let (text, warning) = match result {
            Ok(()) => (STORED_TEXT, false),
            Err(SdError::NoCard) => (NO_CARD_TEXT, true),
            Err(SdError::NotFound | SdError::Failed) => (WRITE_FAILED_TEXT, true),
        };

        // Leaving `Writing` drops the phrase: once it is on the card, the
        // device has no reason to keep holding it.
        self.step = Step::Done {
            from: SdTool::Store,
            text,
            warning,
        };
    }

    /// The outcome of a [`SdRequest::Load`]: the file's bytes, or why there are
    /// none. Ignored unless one is pending.
    pub(crate) fn loaded(&mut self, result: Result<&[u8], SdError>) {
        if !matches!(self.step, Step::Reading) {
            return;
        }

        let failed = |text| Step::Done {
            from: SdTool::Read,
            text,
            warning: true,
        };

        self.step = match result {
            Ok(bytes) => match seed_file::decode(bytes) {
                Some(mnemonic) => Step::Phrase { mnemonic, page: 0 },
                None => failed(INVALID_SEED_TEXT),
            },
            Err(SdError::NoCard) => failed(NO_CARD_TEXT),
            Err(SdError::NotFound) => failed(NOT_FOUND_TEXT),
            Err(SdError::Failed) => failed(READ_FAILED_TEXT),
        };
    }

    fn back_to_pick(&mut self, selected: SdTool) {
        self.step = Step::Pick { selected };
    }

    pub(crate) fn view(&self) -> View<'_> {
        match &self.step {
            Step::Pick { selected } => View::SdPick {
                selected: *selected,
            },
            Step::Length { selected } => View::SeedLengthPick {
                selected: *selected,
            },
            Step::Words { entry, invalid, .. } => View::Words {
                entry,
                label: None,
                notice: invalid.then_some(INVALID_TEXT),
            },
            Step::Writing { .. } => View::Message {
                text: WRITING_TEXT,
                warning: false,
                busy: true,
            },
            Step::Reading => View::Message {
                text: READING_TEXT,
                warning: false,
                busy: true,
            },
            Step::Phrase { mnemonic, page } => View::Phrase {
                mnemonic,
                page: *page,
                editable: false,
            },
            Step::Done { text, warning, .. } => View::Message {
                text,
                warning: *warning,
                busy: false,
            },
        }
    }
}
