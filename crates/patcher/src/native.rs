use std::path::Path;

use crate::error::Result;
use crate::patch::Patch;

type CreatePatchFn = unsafe extern "C" fn() -> *mut PatchBox;

type PatchBox = Box<dyn Patch>;

pub fn load_native_patch(path: impl AsRef<Path>) -> Result<Box<dyn Patch>> {
    let path = path.as_ref();

    // SAFETY: The user-provided shared library is trusted by the bundle author.
    // The library handle is intentionally leaked so loaded code remains valid
    // for the process lifetime — dropping it would invalidate the vtable.
    unsafe {
        let lib = libloading::Library::new(path)?;
        let create: libloading::Symbol<CreatePatchFn> = lib.get(b"stitch_create_patch")?;
        let raw = create();
        let patch = *Box::from_raw(raw);
        std::mem::forget(lib);
        Ok(patch)
    }
}
