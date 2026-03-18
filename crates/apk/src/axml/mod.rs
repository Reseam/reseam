pub mod compiler;
pub mod reader;
pub mod string_pool;
pub mod writer;

pub use compiler::{compile_xml, is_compiled_axml};
pub use reader::{AxmlAttribute, AxmlDocument, AxmlEvent, TypedValue};
pub use string_pool::StringPool;

pub(crate) const CHUNK_XML_DOCUMENT: u16 = 0x0003;
pub(crate) const CHUNK_STRING_POOL: u16 = 0x0001;
pub(crate) const CHUNK_RESOURCE_IDS: u16 = 0x0180;
pub(crate) const CHUNK_START_NAMESPACE: u16 = 0x0100;
pub(crate) const CHUNK_END_NAMESPACE: u16 = 0x0101;
pub(crate) const CHUNK_START_ELEMENT: u16 = 0x0102;
pub(crate) const CHUNK_END_ELEMENT: u16 = 0x0103;

pub(crate) const TYPE_STRING: u8 = 0x03;
pub(crate) const TYPE_INT_DEC: u8 = 0x10;
pub(crate) const TYPE_INT_HEX: u8 = 0x11;
pub(crate) const TYPE_INT_BOOLEAN: u8 = 0x12;
pub(crate) const TYPE_REFERENCE: u8 = 0x01;
