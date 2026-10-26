// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Bytecode compiled from the JavaScript beside it in `fixtures/`.

use reseam_storage::Bytes;

use crate::{Editor, FunctionId, FunctionIndex, HermesImage};

/// An app bundle whose global code defines Metro modules.
pub(crate) const HOST: &[u8] = include_bytes!("../fixtures/host.hbc");
/// The app runtime extension, exporting `isEnabled` and `withProperty`.
pub(crate) const BUNDLED: &[u8] = include_bytes!("../fixtures/bundled.hbc");
/// An extension whose literal references start inside other literal runs.
pub(crate) const LITERALS: &[u8] = include_bytes!("../fixtures/literals.hbc");

pub(crate) fn image(bytes: &[u8]) -> HermesImage {
    HermesImage::parse(Bytes::from_slice(bytes)).expect("fixture parses")
}

/// The host function named `name`.
pub(crate) fn function(host: &HermesImage, index: &FunctionIndex, name: &str) -> FunctionId {
    index
        .find_function(&host.file(), Some(name), &[], None)
        .expect("fixture function exists")
}

pub(crate) fn written(editor: &Editor<'_>) -> Vec<u8> {
    let mut output = Vec::new();
    editor.write(&mut output).expect("file writes");
    output
}
