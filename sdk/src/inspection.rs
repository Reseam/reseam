// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use reseam_model::{
    ApkMetadata, ApkVersion, InspectRequest, InspectResponse, PatchMetadata, PatchOutcome,
    PatchPreset, PatchRequest, PatchSpec, RunEvent,
};
use reseam_patcher::catalog::PatchCatalog;
use reseam_patcher::error::PatcherError;

use crate::apk::OpenedApk;
use crate::bundles::OpenedBundle;
use crate::error::{HostError, Result};
use crate::inputs::Inputs;
use crate::trust::TrustStore;

/// Inspects an optional APK and all requested bundles.
/// APK and trust failures abort inspection; each bundle failure is recorded in
/// its metadata so hosts can display the readable bundles alongside it.
/// Reads signed patch metadata without loading bundle code, regardless of trust.
pub fn inspect(request: &InspectRequest) -> Result<InspectResponse> {
    Ok(PreparedInspection::open(request)?.metadata)
}

/// Retains the opened APK and verified catalogs between inspection and one patch
/// run. Input files must remain unchanged until this object is consumed or dropped,
/// as with [`ApkInspection`](crate::ApkInspection). Inspection never loads executable
/// bundle payloads.
pub struct PreparedInspection {
    request: InspectRequest,
    inputs: Inputs,
    metadata: InspectResponse,
}

impl PreparedInspection {
    pub fn open(request: &InspectRequest) -> Result<Self> {
        let trust = TrustStore::new(&request.trust)?;
        let mut inputs = Inputs::open(
            request.apk_path.as_deref(),
            &request.split_paths,
            &request.bundle_paths,
        )?;
        let apk = inputs.apk.as_mut().map(OpenedApk::metadata).transpose()?;
        let metadata = InspectResponse {
            bundles: inputs
                .bundles
                .iter()
                .map(|bundle| bundle.metadata(&trust))
                .collect(),
            patches: inputs
                .bundles
                .iter()
                .flat_map(OpenedBundle::catalog)
                .map(|spec| patch_metadata(spec, apk.as_ref()))
                .collect(),
            apk,
        };
        Ok(Self {
            request: request.clone(),
            inputs,
            metadata,
        })
    }

    pub fn metadata(&self) -> &InspectResponse {
        &self.metadata
    }

    /// The patch `selector` names, resolved as a [`PatchSelection`](reseam_model::PatchSelection)
    /// resolves it for the inspected package: a reference `<bundle>/<id>`, or an ID
    /// or display name that identifies one patch. `None` when no patch has that name.
    pub fn resolve_patch(&self, selector: &str) -> Result<Option<&PatchMetadata>> {
        let specs: Vec<&PatchSpec> = self
            .metadata
            .patches
            .iter()
            .map(|patch| &patch.spec)
            .collect();
        let package = self
            .metadata
            .apk
            .as_ref()
            .and_then(|apk| apk.package_name.as_deref());
        match PatchCatalog::new(&specs)?.resolve(selector, package) {
            Ok(index) => Ok(Some(&self.metadata.patches[index])),
            Err(PatcherError::UnknownPatch(_)) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Consumes the inspected inputs. Paths and their ordering must match inspection;
    /// selection, signer approvals, signing identity and destination may change.
    /// Trust is checked again and every payload hash is verified before execution.
    pub fn patch(self, request: &PatchRequest, emit: impl FnMut(RunEvent)) -> Result<PatchOutcome> {
        if self.request.apk_path.as_deref() != Some(request.apk_path.as_str())
            || self.request.split_paths != request.split_paths
            || self.request.bundle_paths != request.bundle_paths
        {
            return Err(HostError::InvalidRequest(
                "patch inputs differ from the prepared inspection",
            ));
        }
        crate::run::run(request, self.inputs, emit)
    }
}

fn patch_metadata(spec: &PatchSpec, apk: Option<&ApkMetadata>) -> PatchMetadata {
    let package = apk.and_then(|apk| apk.package_name.as_deref());
    PatchMetadata {
        spec: spec.clone(),
        incompatibility: spec.compatibility.incompatibility(
            package,
            ApkVersion {
                name: apk.and_then(|apk| apk.version_name.as_deref()),
                code: apk.and_then(|apk| apk.version_code),
            },
        ),
        presets: [PatchPreset::Recommended, PatchPreset::All]
            .into_iter()
            .filter(|&preset| spec.in_preset(preset, package))
            .collect(),
    }
}
