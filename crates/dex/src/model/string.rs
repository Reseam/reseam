use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StringIdx(pub u32);

#[derive(Debug, Clone)]
pub struct DexString {
    pub value: Cow<'static, str>,
}

impl DexString {
    pub fn new(s: String) -> Self {
        Self {
            value: Cow::Owned(s),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl std::fmt::Display for DexString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.value)
    }
}
