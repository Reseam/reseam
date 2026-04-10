# stitch-patcher-macros

Proc-macro crate for the `#[stitch_patch]` attribute.

Transforms annotated functions into structs that implement the `Patch` trait. The macro parses patch metadata (name, description, compatible packages/versions, dependencies, default-enabled state) from the attribute and generates the full trait implementation including `execute`, `name`, `description`, `compatible_packages`, `depends_on`, and `options`.

## Usage

```rust
use stitch_patcher::prelude::*;

#[stitch_patch(
    name = "disable-ads",
    description = "Removes video advertisements",
    packages("com.example.app"),
    enabled_by_default = true,
)]
fn execute(ctx: &mut PatchContext) -> Result<()> {
    // patch logic
    Ok(())
}
```

This generates a `DisableAds` struct implementing `Patch` with all metadata wired up.
