use super::encoded_value::EncodedValue;
use super::method_handle::MethodHandleIdx;
use super::proto::ProtoIdx;
use super::string::StringIdx;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallSiteIdx(pub u32);

#[derive(Debug, Clone)]
pub struct CallSiteItem {
    pub bootstrap_method: MethodHandleIdx,
    pub method_name: StringIdx,
    pub method_type: ProtoIdx,
    pub extra_arguments: Vec<EncodedValue>,
}
