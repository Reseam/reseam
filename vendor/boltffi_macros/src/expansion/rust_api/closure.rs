use boltffi_ast::{BaseTrait, FnSig, TypeExpr};
use boltffi_binding::{ClosureForm, HandlePresence};
use syn::Type;

use crate::expansion::{error::Error, rust_api::TypeTokens};

#[derive(Clone, Copy)]
pub enum ClosureSourceForm {
    ImplTrait,
    BoxedDyn,
    NullableBoxedDyn,
    FunctionPointer,
}

#[derive(Clone)]
pub struct Closure {
    form: ClosureSourceForm,
    function: ClosureForm,
    signature: FnSig,
    source: TypeExpr,
}

impl Closure {
    pub fn new(type_expr: &TypeExpr, presence: HandlePresence) -> Result<Self, Error> {
        match presence {
            HandlePresence::Required => Self::required(type_expr),
            HandlePresence::Nullable => match type_expr {
                TypeExpr::Option(inner) => Self::nullable(inner, type_expr),
                _ => Err(Error::SourceSyntaxMismatch(
                    "nullable closure binding requires an optional source closure type",
                )),
            },
            _ => Err(Error::UnsupportedExpansion("unknown closure presence")),
        }
    }

    pub const fn form(&self) -> ClosureSourceForm {
        self.form
    }

    pub const fn function(&self) -> ClosureForm {
        self.function
    }

    pub const fn signature(&self) -> &FnSig {
        &self.signature
    }

    pub fn ty(&self) -> Result<Type, Error> {
        TypeTokens::new(&self.source).map(TypeTokens::into_type)
    }

    fn required(type_expr: &TypeExpr) -> Result<Self, Error> {
        match type_expr {
            TypeExpr::FnPtr(signature) => Ok(Self {
                form: ClosureSourceForm::FunctionPointer,
                function: ClosureForm::FunctionPointer,
                signature: signature.as_ref().clone(),
                source: type_expr.clone(),
            }),
            TypeExpr::ImplTrait(bounds) => match &bounds.base {
                BaseTrait::Function(function_trait) => Ok(Self {
                    form: ClosureSourceForm::ImplTrait,
                    function: ClosureForm::from(function_trait.kind),
                    signature: function_trait.signature.clone(),
                    source: type_expr.clone(),
                }),
                BaseTrait::Named { .. } => Err(Error::SourceSyntaxMismatch(
                    "source type is not an inline closure",
                )),
            },
            TypeExpr::Boxed(inner) => match inner.as_ref() {
                TypeExpr::Dyn(bounds) => match &bounds.base {
                    BaseTrait::Function(function_trait) => Ok(Self {
                        form: ClosureSourceForm::BoxedDyn,
                        function: ClosureForm::from(function_trait.kind),
                        signature: function_trait.signature.clone(),
                        source: type_expr.clone(),
                    }),
                    BaseTrait::Named { .. } => Err(Error::SourceSyntaxMismatch(
                        "source closure type is not a boxed closure trait object",
                    )),
                },
                _ => Err(Error::SourceSyntaxMismatch(
                    "source closure type is not a boxed closure trait object",
                )),
            },
            _ => Err(Error::SourceSyntaxMismatch(
                "source type is not an inline closure",
            )),
        }
    }

    fn nullable(inner: &TypeExpr, source: &TypeExpr) -> Result<Self, Error> {
        match inner {
            TypeExpr::Boxed(boxed) => match boxed.as_ref() {
                TypeExpr::Dyn(bounds) => match &bounds.base {
                    BaseTrait::Function(function_trait) => Ok(Self {
                        form: ClosureSourceForm::NullableBoxedDyn,
                        function: ClosureForm::from(function_trait.kind),
                        signature: function_trait.signature.clone(),
                        source: source.clone(),
                    }),
                    BaseTrait::Named { .. } => Err(Error::SourceSyntaxMismatch(
                        "nullable closure source type is not Option<Box<dyn Fn*>>",
                    )),
                },
                _ => Err(Error::SourceSyntaxMismatch(
                    "nullable closure source type is not Option<Box<dyn Fn*>>",
                )),
            },
            _ => Err(Error::SourceSyntaxMismatch(
                "nullable closure source type is not Option<Box<dyn Fn*>>",
            )),
        }
    }
}
