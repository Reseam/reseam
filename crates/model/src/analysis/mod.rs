// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What read-only analysis of an APK reports: the overview, text matches,
//! references and the views `show` prints.

mod check;
mod diff;
mod inspect;
mod overview;
mod refs;
mod show;
mod symbol;
mod text;

pub use check::{PatchCheck, ResolveReport, ResolveRequest};
pub use diff::{BodyDiff, Change, DiffLine};
pub use inspect::{
    CompileError, Inspected, KotlinCompiler, Snippet, TargetInspection, TargetResolution,
};
pub use overview::{
    CodeGroup, DexLanguage, EntryPoints, Marker, Overview, PackageGroup, Runtime, RuntimeKind,
    SdkCategory, SdkUse,
};
pub use refs::{
    CallNode, CallStop, CallTree, CodeSite, RefKind, RefSite, RefTarget, References, Relation,
    XmlUse,
};
pub use show::{
    ApkView, BodyLine, ClassView, ComponentKind, ComponentView, ConfigValue, FieldView, HintSource,
    LineKind, MethodSummary, MethodView, NameHint, NamedMember, ResourceId, ResourceView,
};
pub use symbol::{ClassRef, DescriptorError, FieldRef, Member, MethodRef};
pub use text::{DexString, StringResource, TextMatches, XmlHit};
