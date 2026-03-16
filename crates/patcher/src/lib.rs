pub mod bundle;
pub mod context;
pub mod engine;
pub mod error;
#[cfg(feature = "lua")]
mod lua_insn;
#[cfg(feature = "lua")]
pub mod lua;
#[cfg(feature = "native")]
pub mod native;
pub mod patch;

pub use stitch_apk;
pub use stitch_apk::stitch_dex;

pub mod prelude {
    pub use crate::context::PatchContext;
    pub use crate::error::{PatcherError, Result};
    pub use crate::patch::{Compatibility, Patch};
    pub use stitch_patcher_macros::stitch_patch;
}

pub use stitch_patcher_macros::stitch_patch;
