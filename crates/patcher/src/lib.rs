pub mod bundle;
pub mod context;
pub mod dependency;
pub mod engine;
pub mod error;
#[cfg(feature = "kotlin")]
pub mod kotlin;
pub mod log;
pub mod options;
pub mod patch;

pub use stitch_apk;
pub use stitch_apk::stitch_dex;

pub mod prelude {
    pub use crate::context::PatchContext;
    pub use crate::engine::ExecutionPlan;
    pub use crate::error::{PatcherError, Result};
    pub use crate::log::{LogEntry, LogLevel, PatchLog};
    pub use crate::options::{OptionDeclaration, OptionType, OptionValue, PatchOptions};
    pub use crate::patch::{Compatibility, Patch};
    pub use stitch_patcher_macros::stitch_patch;
}

pub use stitch_patcher_macros::stitch_patch;
