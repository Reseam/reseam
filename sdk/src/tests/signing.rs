// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_model::{Problem, SdkError, SigningKeyMaterial};
use reseam_sign::SigningKey;

use crate::{export_pkcs12, import_pkcs12};

#[test]
fn a_backup_opened_with_the_wrong_password_reports_a_locked_keystore() {
    let key = SigningKey::generate().unwrap();
    let material = SigningKeyMaterial {
        key: key.pkcs8_der().to_vec(),
        cert: key.certificate_der().to_vec(),
    };
    let backup = export_pkcs12(&material, "right").unwrap();
    assert_eq!(import_pkcs12(&backup, "right").unwrap(), material);
    let error = SdkError::from(import_pkcs12(&backup, "wrong").unwrap_err());
    assert_eq!(error.problem, Problem::KeystoreLocked);
}
