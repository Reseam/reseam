// Generated from PatchIndexFormat.kt. Do not edit.

pub(crate) const PATCH_INDEX: &str = "META-INF/reseam/patches.json";

#[cfg(feature = "kotlin")]
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub(crate) enum MemberKind {
    #[serde(rename = "field")]
    Field,
    #[serde(rename = "method")]
    Method,
}

#[cfg(feature = "kotlin")]
#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub(crate) struct Declaration {
    #[serde(rename = "class")]
    pub(crate) class_name: String,
    pub(crate) owner: String,
    pub(crate) member: String,
    pub(crate) kind: MemberKind,
    pub(crate) id: String,
}
