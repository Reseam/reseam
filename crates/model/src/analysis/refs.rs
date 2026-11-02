// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

use super::show::ResourceId;
use super::symbol::{ClassRef, Member, MethodRef};

/// One instruction of one method.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CodeSite {
    pub method: MethodRef,
    pub index: u32,
    /// The instruction as smali.
    pub excerpt: String,
}

/// What `refs` looks for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RefTarget {
    Member(Member),
    /// A string constant, matched exactly.
    String(String),
    /// A number literal, resource ids included.
    Number(i64),
    /// A resource: code loading its id, and XML using it.
    Resource(u32),
    /// Every member of a package and its subpackages, in Java form.
    Package(String),
}

/// An instruction that references the target.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RefSite {
    pub site: CodeSite,
    pub kind: RefKind,
    /// What the instruction names: for a package, the package member it uses.
    pub target: Option<Member>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    Get,
    Put,
    Call,
    NewInstance,
    NewArray,
    CheckCast,
    InstanceOf,
    ConstClass,
    LoadString,
    LoadNumber,
}

/// A reference that is not an instruction: a declaration or an XML use.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Relation {
    /// A method that overrides or implements the target method.
    Override {
        method: MethodRef,
    },
    /// A method the target overrides or implements; calls to it can reach the target.
    Overridden {
        method: MethodRef,
    },
    /// An interface method the target implements along with so many other classes, like a
    /// Kotlin lambda's `invoke`, that a call through it says nothing about which one runs.
    /// Its calls are left out.
    Shared {
        method: MethodRef,
        implementers: u32,
    },
    Subclass {
        class: ClassRef,
    },
    Implementer {
        class: ClassRef,
    },
    /// A field declared with the target type.
    FieldOfType {
        member: Member,
    },
    /// A method that takes or returns the target type.
    Signature {
        method: MethodRef,
    },
    /// An XML element in a resource file that uses the target resource.
    Xml {
        file: String,
        element: String,
    },
    /// A manifest element that uses the target resource.
    Manifest {
        element: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct References {
    pub sites: Vec<RefSite>,
    pub relations: Vec<Relation>,
}

/// The callers of a method, followed to a depth: a tree in pre-order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CallTree {
    pub root: MethodRef,
    /// Why the root has no callers, when it has none.
    pub stop: Option<CallStop>,
    pub nodes: Vec<CallNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CallNode {
    /// 1 for direct callers of the root.
    pub depth: u32,
    /// The call site in the caller.
    pub site: CodeSite,
    /// Why the tree stops here, when it does before the depth limit.
    pub stop: Option<CallStop>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CallStop {
    /// Nothing in the app calls the method. Its class extends or implements
    /// these types from outside the APK, through which the system calls it.
    NoCallers { external_supertypes: Vec<ClassRef> },
    /// The method is already on the path, so the call is recursive.
    Cycle,
    /// The app calls the method only through `through`, an interface method many classes
    /// implement. Where its class is created says where it comes from.
    Shared {
        through: MethodRef,
        implementers: u32,
    },
}

/// Where an id shows up in resource XML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct XmlUse {
    /// The file's path in the APK.
    pub file: String,
    /// The file as a resource, when the table names it.
    pub resource: Option<ResourceId>,
    pub element: String,
    /// The element's `android:id`, when it has one.
    pub view_id: Option<ResourceId>,
}
