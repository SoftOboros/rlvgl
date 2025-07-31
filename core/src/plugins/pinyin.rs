//! Minimal Pinyin input method dictionary.
use alloc::vec::Vec;

/// Simplified Pinyin input method that maps Latin pinyin syllables to Chinese
/// characters. The dictionary here only covers a small subset of words used by
/// the tests and is not exhaustive.
pub struct PinyinInputMethod;

/// Static mapping table derived from the reference LVGL implementation.
const DICT: &[(&str, &str)] = &[
    ("zhong", "中种終重種眾"),
    ("guo", "果国裏菓國過"),
    ("ai", "愛"),
];

impl PinyinInputMethod {
    /// Look up candidate characters for a given pinyin syllable.
    ///
    /// The reference LVGL implementation returns matches even when the
    /// provided input is only a prefix of the stored syllable. For example
    /// typing `"zh"` will return the candidates for `"zhong"`.
    pub fn candidates(&self, input: &str) -> Option<Vec<char>> {
        if input.is_empty() {
            return None;
        }
        // In LVGL the search routine ignores inputs starting with i/u/v or a
        // space character. Mirror that behaviour for the limited dictionary
        // here.
        if matches!(input.chars().next(), Some('i' | 'u' | 'v' | ' ')) {
            return None;
        }

        for &(py, chars) in DICT {
            if py.starts_with(input) {
                return Some(chars.chars().collect());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_basic_and_prefix() {
        let ime = PinyinInputMethod;
        let result_full = ime.candidates("zhong").unwrap();
        assert_eq!(result_full[0], '中');

        // Partial input should behave the same as the C implementation and
        // return the same candidate list.
        let result_prefix = ime.candidates("zho").unwrap();
        assert_eq!(result_prefix, result_full);

        // Single letter input also resolves to the first matching entry.
        let single = ime.candidates("g").unwrap();
        assert_eq!(single[0], '果');
    }

    #[test]
    fn unknown_returns_none() {
        let ime = PinyinInputMethod;
        assert!(ime.candidates("foobar").is_none());
    }
}
