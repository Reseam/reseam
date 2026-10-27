// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use thiserror::Error;

use crate::FunctionId;

/// A malformed file, an edit Reseam cannot make, or an output failure.
#[derive(Debug, Error)]
pub enum HermesError {
    #[error("not a Hermes bytecode file")]
    Magic,
    #[error("unsupported Hermes bytecode version {0}; supported: 98")]
    Version(u32),
    /// Bytecode that breaks the format, at a byte offset of the file it was read from.
    #[error("invalid Hermes bytecode at offset {offset}: {reason}")]
    Invalid { offset: usize, reason: &'static str },
    #[error("Hermes function {} does not exist", .0.0)]
    NoFunction(FunctionId),
    #[error("Hermes string {0} does not exist")]
    NoString(u32),
    #[error("unsupported Hermes function {}: {reason}", .function.0)]
    Unsupported {
        function: FunctionId,
        reason: Unsupported,
    },
    #[error("cannot link Hermes module: {0}")]
    Link(LinkError),
    /// An edit that would grow the file past what its format holds.
    #[error("Hermes file would exceed {0}")]
    Limit(&'static str),
    #[error(
        "Hermes lookup matched {count} functions: name={name:?}, strings={strings:?}, parameters={parameters:?}"
    )]
    Match {
        name: Option<String>,
        strings: Vec<String>,
        parameters: Option<u32>,
        count: usize,
    },
    #[error(transparent)]
    Truncated(#[from] reseam_bytes::Truncated),
    #[error("Hermes output: {0}")]
    Io(#[from] std::io::Error),
}

/// Why a function cannot be hooked, or an import cannot be resolved to it.
#[derive(Debug, Error)]
pub enum Unsupported {
    #[error("it is the global function")]
    Global,
    #[error("it is a generator, async function or constructor, or uses new.target or eval")]
    Stateful,
    #[error("it is not reachable from the app's global closures")]
    Unreachable,
    #[error("its closure environment at byte {0} cannot be proven")]
    UnprovenEnvironment(u32),
    #[error("its closures sit at different environment depths")]
    EnvironmentDepths,
    #[error("its private environment cannot be resolved")]
    UnresolvedEnvironment,
    #[error("its environment at byte {0} escapes the private root")]
    EscapedEnvironment(u32),
    #[error("its private environment is deeper than 255 scopes")]
    DeepEnvironment,
    #[error("it was replaced, so the closures a hook needs no longer exist")]
    Replaced,
    #[error("it creates closures that a hook needs")]
    NeededClosures,
    #[error("it is created outside every Metro module")]
    OutsideMetro,
    #[error("it is created in several Metro modules")]
    SeveralMetro,
    #[error("the global code defines no Metro modules")]
    NoMetro,
    #[error("the Metro module definition at byte {0} has no provable factory and ID")]
    UnprovenMetro(u32),
    #[error("it is the factory of several Metro modules")]
    SharedFactory,
    #[error("an edited instruction's operands fit no encoding")]
    Encoding,
    #[error("the hook needs more than 255 registers")]
    Registers,
    #[error("the hook reads more than 255 properties")]
    Properties,
    #[error("malformed hook code: {0}")]
    Malformed(&'static str),
    #[error("the module does not export {0:?}")]
    Export(String),
    #[error("the hook calls a module this editor did not link")]
    Module,
}

/// Why a module cannot be linked.
#[derive(Debug, Error)]
pub enum LinkError {
    #[error("it has CommonJS segments; link standalone execution bytecode")]
    Segmented,
    #[error("it assumes static builtins and the app does not")]
    StaticBuiltins,
    #[error("it declares globals; compile an IIFE returning exports")]
    Globals,
    #[error("it declares no imports array of string literals")]
    Imports,
    #[error("it declares no link function")]
    Link,
    #[error("it imports {0:?}, which is not resolved")]
    Unresolved(String),
    #[error("{0:?} is resolved but not imported once by the module")]
    Unimported(String),
}

pub type Result<T> = std::result::Result<T, HermesError>;

pub(crate) fn invalid(offset: usize, reason: &'static str) -> HermesError {
    HermesError::Invalid { offset, reason }
}

pub(crate) fn unsupported(function: FunctionId, reason: Unsupported) -> HermesError {
    HermesError::Unsupported { function, reason }
}
