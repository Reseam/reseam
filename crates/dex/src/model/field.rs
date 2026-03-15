use super::string::StringIdx;
use super::types::TypeIdx;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldIdx(pub u32);

#[derive(Debug, Clone)]
pub struct FieldId {
    pub class: TypeIdx,
    pub type_: TypeIdx,
    pub name: StringIdx,
}
