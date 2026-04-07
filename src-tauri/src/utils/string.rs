pub trait StringUtils {
    /// 分割出空格字符
    ///
    /// # Examples
    ///
    /// ```
    /// let s = "hello".to_string();
    /// let s = s.add_between_chars(" ");
    /// assert_eq!(s, "h e l l o");
    /// ```
    fn add_between_chars(&self, between: &str) -> String;
    fn collapse_whitespace(&self) -> String;
    fn build_default_vosk_grammar(&self) -> Option<String>;
    fn normalize_text_for_matching(&self) -> String;
}

fn is_cjk_like_char(ch: char) -> bool {
    matches!(
        ch as u32,
        0x2E80..=0x2EFF
            | 0x2F00..=0x2FDF
            | 0x3040..=0x309F
            | 0x30A0..=0x30FF
            | 0x3100..=0x312F
            | 0x31A0..=0x31BF
            | 0x31C0..=0x31EF
            | 0x31F0..=0x31FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF66..=0xFF9D
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0x2CEB0..=0x2EBEF
            | 0x30000..=0x3134F
    )
}

impl StringUtils for str {
    fn add_between_chars(&self, between: &str) -> String {
        self.chars()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(between)
    }

    fn collapse_whitespace(&self) -> String {
        self.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn build_default_vosk_grammar(&self) -> Option<String> {
        let collapsed = self.collapse_whitespace();
        if collapsed.is_empty() {
            return None;
        }

        let mut tokens = Vec::new();
        let mut current = String::new();

        for ch in collapsed.chars() {
            if ch.is_whitespace() {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                continue;
            }

            if is_cjk_like_char(ch) {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                tokens.push(ch.to_string());
            } else {
                current.push(ch);
            }
        }

        if !current.is_empty() {
            tokens.push(current);
        }

        Some(tokens.join(" "))
    }

    fn normalize_text_for_matching(&self) -> String {
        self.collapse_whitespace()
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .flat_map(|ch| ch.to_lowercase())
            .collect()
    }
}

impl StringUtils for String {
    fn add_between_chars(&self, between: &str) -> String {
        self.as_str().add_between_chars(between)
    }

    fn collapse_whitespace(&self) -> String {
        self.as_str().collapse_whitespace()
    }

    fn build_default_vosk_grammar(&self) -> Option<String> {
        self.as_str().build_default_vosk_grammar()
    }

    fn normalize_text_for_matching(&self) -> String {
        self.as_str().normalize_text_for_matching()
    }
}

pub trait StringOptionUtils {
    fn is_empty(&self) -> bool;
}

impl StringOptionUtils for Option<String> {
    fn is_empty(&self) -> bool {
        if let Some(s) = self {
            s.is_empty()
        } else {
            true
        }
    }
}
