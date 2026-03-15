use super::*;

#[test]
fn resolve_class_data_rejects_out_of_bounds_index() {
    let mut dex = DexFile::new(empty_test_header());
    dex.lazy_class_data_offsets = Some(Vec::new());

    assert!(matches!(
        dex.resolve_class_data(0),
        Err(DexError::IndexOutOfBounds {
            index_type: "class",
            index: 0,
            table_size: 0,
        })
    ));
}

#[test]
fn intern_descriptors_return_errors() {
    let mut dex = DexFile::new(empty_test_header());

    assert!(matches!(
        dex.intern_proto("(V)V"),
        Err(DexError::InvalidDescriptor {
            kind: "method descriptor",
            ..
        })
    ));
    assert!(matches!(
        dex.intern_method("not-a-type", "name", "()V"),
        Err(DexError::InvalidDescriptor {
            kind: "class descriptor",
            ..
        })
    ));
    assert!(matches!(
        dex.intern_field("Lcom/example/Test;", "value", "bad"),
        Err(DexError::InvalidDescriptor {
            kind: "field descriptor",
            ..
        })
    ));
}
