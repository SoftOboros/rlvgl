use alloc::string::String;
use pinyin::ToPinyin;

/// Simple Pinyin input method service.
pub struct PinyinInputMethod;

impl PinyinInputMethod {
    /// Convert Chinese text to plain pinyin separated by spaces.
    pub fn transliterate(&self, input: &str) -> String {
        let mut out = String::new();
        for py in input.to_pinyin() {
            if let Some(py) = py {
                out.push_str(py.plain());
            } else {
                out.push('?');
            }
            out.push(' ');
        }
        String::from(out.trim_end())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transliterate_basic() {
        let ime = PinyinInputMethod;
        let out = ime.transliterate("中国");
        assert_eq!(out, "zhong guo");
    }
}
