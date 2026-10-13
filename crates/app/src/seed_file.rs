//! How a phrase is laid out in the file on the SD card.
//!
//! Plain text, one line: the words in lower case, separated by single spaces.
//! That is what any wallet's import box takes, so the card can be read on a
//! computer as well as on the device. **Not encrypted**: anyone holding the
//! card holds the phrase.
//!
//! Here rather than in the firmware so the format, and above all the check on
//! the way back in, is tested on the host. The firmware only moves bytes.

use heapless::{String, Vec};

use sporos_core::bip39::{self, Mnemonic, SeedLength, Word, MAX_WORD_COUNT_TOTAL};

/// The file both tools use. 8.3, because the FAT driver has no long names.
pub const SEED_FILE: &str = "SEED.TXT";

/// Room for the longest phrase: 24 words of up to eight letters, the spaces
/// between them and the newline, with some to spare.
pub const SEED_FILE_CAPACITY: usize = 256;

/// The file contents for `mnemonic`.
pub fn encode(mnemonic: &Mnemonic) -> String<SEED_FILE_CAPACITY> {
    let mut text = String::new();

    for (index, word) in mnemonic.words().iter().enumerate() {
        if index > 0 {
            text.push(' ').expect("sized for the longest phrase");
        }
        for letter in word.chars() {
            text.push(letter.to_ascii_lowercase())
                .expect("sized for the longest phrase");
        }
    }
    text.push('\n').expect("sized for the longest phrase");

    text
}

/// Reads a file back, accepting it only if it is a whole phrase of a length the
/// device knows and its checksum holds — the same check a typed phrase has to
/// pass. Any whitespace separates words and any case is accepted, so a file
/// written by hand on a computer reads too.
pub fn decode(bytes: &[u8]) -> Option<Mnemonic> {
    let text = core::str::from_utf8(bytes).ok()?;

    let mut words: Vec<Word, MAX_WORD_COUNT_TOTAL> = Vec::new();
    for word in text.split_whitespace() {
        words.push(Word::try_from(word).ok()?).ok()?;
    }

    let length = SeedLength::ALL
        .into_iter()
        .find(|length| length.total_words() == words.len())?;

    bip39::parse(length, &words)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The all-zero entropy phrases, the first test vectors in BIP-39.
    const ZERO_12: &str = "abandon abandon abandon abandon abandon abandon \
                           abandon abandon abandon abandon abandon about";
    const ZERO_24: &str = "abandon abandon abandon abandon abandon abandon \
                           abandon abandon abandon abandon abandon abandon \
                           abandon abandon abandon abandon abandon abandon \
                           abandon abandon abandon abandon abandon art";

    #[test]
    fn a_phrase_survives_the_round_trip() {
        for text in [ZERO_12, ZERO_24] {
            let mnemonic = decode(text.as_bytes()).expect("a valid phrase");
            let encoded = encode(&mnemonic);

            assert_eq!(decode(encoded.as_bytes()), Some(mnemonic));
        }
    }

    #[test]
    fn the_file_is_one_lower_case_line() {
        let mnemonic = decode(ZERO_12.as_bytes()).expect("a valid phrase");
        let encoded = encode(&mnemonic);

        assert!(encoded.ends_with("about\n"));
        assert_eq!(encoded.lines().count(), 1);
        assert!(!encoded.chars().any(|c| c.is_ascii_uppercase()));
        assert_eq!(encoded.split(' ').count(), 12);
    }

    #[test]
    fn upper_case_and_stray_whitespace_are_accepted() {
        let shouted = ZERO_12.to_ascii_uppercase().replace(' ', "\n  ");

        assert!(decode(shouted.as_bytes()).is_some());
    }

    #[test]
    fn a_bad_checksum_is_refused() {
        let wrong = ZERO_12.replace("about", "abandon");

        assert_eq!(decode(wrong.as_bytes()), None);
    }

    #[test]
    fn a_word_not_in_the_list_is_refused() {
        let wrong = ZERO_12.replacen("abandon", "bitcoin", 1);

        assert_eq!(decode(wrong.as_bytes()), None);
    }

    #[test]
    fn a_length_the_device_does_not_know_is_refused() {
        let words: std::vec::Vec<&str> = ZERO_24.split_whitespace().collect();

        for count in [0, 1, 11, 13, 18, 25] {
            let text = std::iter::repeat_n("abandon", count)
                .collect::<std::vec::Vec<_>>()
                .join(" ");
            assert_eq!(decode(text.as_bytes()), None, "{count} words");
        }

        // Too many to even hold must fail cleanly, not panic.
        let long = words.repeat(3).join(" ");
        assert_eq!(decode(long.as_bytes()), None);
    }

    #[test]
    fn bytes_that_are_not_text_are_refused() {
        assert_eq!(decode(&[0xff, 0xfe, 0x00]), None);
    }
}
