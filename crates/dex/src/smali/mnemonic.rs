// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

/// Width and variant suffixes as the catalogue spells them, and as smali
/// writes them after a slash.
const SUFFIXES: [(&str, &str); 7] = [
    ("_RANGE", "range"),
    ("2_ADDR", "2addr"),
    ("_LIT16", "lit16"),
    ("_LIT8", "lit8"),
    ("_FROM16", "from16"),
    ("_HIGH16", "high16"),
    ("_JUMBO", "jumbo"),
];

/// The smali mnemonic for a catalogue opcode name: `INVOKE_VIRTUAL_RANGE` is
/// `invoke-virtual/range`, `CONST_WIDE16` is `const-wide/16`.
pub(super) fn mnemonic(opname: &str) -> String {
    match opname {
        "PACKED_SWITCH_PAYLOAD" => return ".packed-switch".to_owned(),
        "SPARSE_SWITCH_PAYLOAD" => return ".sparse-switch".to_owned(),
        "FILL_ARRAY_DATA_PAYLOAD" => return ".array-data".to_owned(),
        "RAW_INSTRUCTION" => return ".raw".to_owned(),
        // The 16-bit form is the original `rsub-int`; only the 8-bit one is marked.
        "RSUB_INT_LIT16" => return "rsub-int".to_owned(),
        _ => {}
    }
    let opname = opname
        .replacen("CMP_L_", "CMPL_", 1)
        .replacen("CMP_G_", "CMPG_", 1);
    let (stem, suffix) = SUFFIXES
        .iter()
        .find_map(|(spelled, suffix)| {
            opname
                .strip_suffix(spelled)
                .map(|stem| (stem.to_owned(), Some((*suffix).to_owned())))
        })
        .unwrap_or_else(|| {
            let stem = opname.trim_end_matches(|c: char| c.is_ascii_digit());
            let width = &opname[stem.len()..];
            (
                stem.to_owned(),
                (!width.is_empty()).then(|| width.to_owned()),
            )
        });
    let stem = stem.to_ascii_lowercase().replace('_', "-");
    match suffix {
        Some(suffix) => format!("{stem}/{suffix}"),
        None => stem,
    }
}

#[cfg(test)]
mod tests {
    use super::mnemonic;

    #[test]
    fn catalogue_names_become_smali_mnemonics() {
        for (opname, expected) in [
            ("NOP", "nop"),
            ("MOVE_FROM16", "move/from16"),
            ("MOVE_WIDE16", "move-wide/16"),
            ("MOVE_RESULT_OBJECT", "move-result-object"),
            ("CONST4", "const/4"),
            ("CONST_HIGH16", "const/high16"),
            ("CONST_WIDE32", "const-wide/32"),
            ("CONST_WIDE_HIGH16", "const-wide/high16"),
            ("CONST_STRING_JUMBO", "const-string/jumbo"),
            ("GOTO32", "goto/32"),
            ("CMP_L_FLOAT", "cmpl-float"),
            ("CMP_G_DOUBLE", "cmpg-double"),
            ("CMP_LONG", "cmp-long"),
            ("IF_NEZ", "if-nez"),
            ("INVOKE_VIRTUAL_RANGE", "invoke-virtual/range"),
            ("FILLED_NEW_ARRAY_RANGE", "filled-new-array/range"),
            ("ADD_INT2_ADDR", "add-int/2addr"),
            ("SHL_LONG2_ADDR", "shl-long/2addr"),
            ("RSUB_INT_LIT16", "rsub-int"),
            ("RSUB_INT_LIT8", "rsub-int/lit8"),
            ("ADD_INT_LIT16", "add-int/lit16"),
            ("INT_TO_LONG", "int-to-long"),
            ("INVOKE_POLYMORPHIC_RANGE", "invoke-polymorphic/range"),
            ("PACKED_SWITCH_PAYLOAD", ".packed-switch"),
        ] {
            assert_eq!(mnemonic(opname), expected, "{opname}");
        }
    }
}
