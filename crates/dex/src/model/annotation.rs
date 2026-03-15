use super::encoded_value::EncodedValue;
use super::field::FieldIdx;
use super::method::MethodIdx;
use super::string::StringIdx;
use super::types::TypeIdx;

#[derive(Debug, Clone)]
pub struct AnnotationsDirectory {
    pub class_annotations: Vec<AnnotationItem>,
    pub field_annotations: Vec<(FieldIdx, Vec<AnnotationItem>)>,
    pub method_annotations: Vec<(MethodIdx, Vec<AnnotationItem>)>,
    pub parameter_annotations: Vec<(MethodIdx, Vec<Vec<AnnotationItem>>)>,
}

#[derive(Debug, Clone)]
pub struct AnnotationItem {
    pub visibility: AnnotationVisibility,
    pub type_: TypeIdx,
    pub elements: Vec<AnnotationElement>,
}

#[derive(Debug, Clone)]
pub struct AnnotationElement {
    pub name: StringIdx,
    pub value: EncodedValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationVisibility {
    Build,
    Runtime,
    System,
}

impl AnnotationVisibility {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x00 => Some(Self::Build),
            0x01 => Some(Self::Runtime),
            0x02 => Some(Self::System),
            _ => None,
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            Self::Build => 0x00,
            Self::Runtime => 0x01,
            Self::System => 0x02,
        }
    }
}
