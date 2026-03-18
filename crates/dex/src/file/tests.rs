use super::*;
use crate::DexError;

#[test]
fn resolve_class_data_rejects_out_of_bounds_index() {
    let mut dex = DexFile::new(empty_test_header());
    dex.lazy_class_data_offsets = Some(Vec::new());

    assert!(matches!(
        dex.resolve_class_data(0),
        Err(DexError::Invalid {
            section: "class",
            ..
        })
    ));
}

#[test]
fn intern_descriptors_return_errors() {
    let mut dex = DexFile::new(empty_test_header());

    assert!(matches!(
        dex.intern_proto("(V)V"),
        Err(DexError::Invalid {
            section: "method descriptor",
            ..
        })
    ));
    assert!(matches!(
        dex.intern_method("not-a-type", "name", "()V"),
        Err(DexError::Invalid {
            section: "class descriptor",
            ..
        })
    ));
    assert!(matches!(
        dex.intern_field("Lcom/example/Test;", "value", "bad"),
        Err(DexError::Invalid {
            section: "field descriptor",
            ..
        })
    ));
}
