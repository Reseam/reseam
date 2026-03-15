use super::string::StringIdx;
use super::types::TypeIdx;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProtoIdx(pub u16);

#[derive(Debug, Clone)]
pub struct Prototype {
    pub shorty: StringIdx,
    pub return_type: TypeIdx,
    pub parameters: Vec<TypeIdx>,
}
