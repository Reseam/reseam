use super::proto::ProtoIdx;
use super::string::StringIdx;
use super::types::TypeIdx;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MethodIdx(pub u32);

#[derive(Debug, Clone)]
pub struct MethodId {
    pub class: TypeIdx,
    pub proto: ProtoIdx,
    pub name: StringIdx,
}
