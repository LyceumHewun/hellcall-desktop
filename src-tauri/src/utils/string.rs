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
}

impl StringUtils for String {
    fn add_between_chars(&self, between: &str) -> String {
        self.chars()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(between)
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
