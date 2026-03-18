#[allow(
    dead_code,
    clippy::all,
    unused_unsafe,
    unused_imports,
    missing_docs,
    non_snake_case,
    non_camel_case_types
)]
#[doc(hidden)]
pub mod bindings;

pub mod insn;
pub mod opcode;

pub use bindings::stitch::patch::bytecode;
pub use bindings::stitch::patch::log;
pub use bindings::stitch::patch::manifest;
pub use bindings::stitch::patch::options;
pub use bindings::stitch::patch::resources;
pub use bindings::stitch::patch::types;
pub use bindings::stitch::patch::xml;

pub use bindings::Guest;
pub use bindings::PatchMetadata;
pub use bindings::OptionDeclaration;
pub use types::Compatibility;

pub use stitch_patch_macros::stitch_patch;

pub mod prelude {
    pub use crate::bytecode;
    pub use crate::log;
    pub use crate::manifest;
    pub use crate::options;
    pub use crate::resources;
    pub use crate::types;
    pub use crate::xml;
    pub use crate::opcode;
    pub use crate::types::{
        Instruction, MethodRef, FieldRef, AccessFlags,
        Fingerprint, FingerprintMatch, MethodInfo, ClassInfo,
        NewMethod, NewField, OptionType,
        TryItem, TypedCatch, CatchHandler,
        EncodedValue, AnnotationVisibility, AnnotationElement, AnnotationItem,
    };
    pub use stitch_patch_macros::stitch_patch;
}

#[macro_export]
macro_rules! export {
    ($ty:ident) => {
        $crate::__export_stitch_patch_impl!($ty with_types_in $crate::bindings);
    };
}
