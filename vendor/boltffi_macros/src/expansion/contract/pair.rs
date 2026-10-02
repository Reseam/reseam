use boltffi_ast::{
    ClassDef, ConstantDef, DeclarationId as SourceDeclarationId, EnumDef, FunctionDef, RecordDef,
    StreamDef, TraitDef,
};
use boltffi_binding::{
    CallbackDecl, ClassDecl, ConstantDecl, Decl, EnumDecl, FunctionDecl, RecordDecl, StreamDecl,
    Surface,
};

use crate::expansion::error::Error;

pub enum SourceDeclaration<'lowered> {
    Record(&'lowered RecordDef),
    Enum(&'lowered EnumDef),
    Function(&'lowered FunctionDef),
    Class(&'lowered ClassDef),
    Callback(&'lowered TraitDef),
    Stream(&'lowered StreamDef),
    Constant(&'lowered ConstantDef),
}

impl<'lowered> SourceDeclaration<'lowered> {
    pub fn id(&self) -> SourceDeclarationId {
        match self {
            Self::Record(source) => SourceDeclarationId::Record(source.id.clone()),
            Self::Enum(source) => SourceDeclarationId::Enum(source.id.clone()),
            Self::Function(source) => SourceDeclarationId::Function(source.id.clone()),
            Self::Class(source) => SourceDeclarationId::Class(source.id.clone()),
            Self::Callback(source) => SourceDeclarationId::Trait(source.id.clone()),
            Self::Stream(source) => SourceDeclarationId::Stream(source.id.clone()),
            Self::Constant(source) => SourceDeclarationId::Constant(source.id.clone()),
        }
    }

    pub fn pair<S: Surface>(
        self,
        binding: &'lowered Decl<S>,
    ) -> Result<PairedDeclaration<'lowered, S>, Error> {
        match (self, binding) {
            (Self::Record(source), Decl::Record(binding)) => Ok(PairedDeclaration::Record(
                DeclarationPair::new(source, binding.as_ref()),
            )),
            (Self::Enum(source), Decl::Enum(binding)) => Ok(PairedDeclaration::Enum(
                DeclarationPair::new(source, binding.as_ref()),
            )),
            (Self::Function(source), Decl::Function(binding)) => Ok(PairedDeclaration::Function(
                DeclarationPair::new(source, binding.as_ref()),
            )),
            (Self::Class(source), Decl::Class(binding)) => Ok(PairedDeclaration::Class(
                DeclarationPair::new(source, binding.as_ref()),
            )),
            (Self::Callback(source), Decl::Callback(binding)) => Ok(PairedDeclaration::Callback(
                DeclarationPair::new(source, binding.as_ref()),
            )),
            (Self::Stream(source), Decl::Stream(binding)) => Ok(PairedDeclaration::Stream(
                DeclarationPair::new(source, binding.as_ref()),
            )),
            (Self::Constant(source), Decl::Constant(binding)) => Ok(PairedDeclaration::Constant(
                DeclarationPair::new(source, binding.as_ref()),
            )),
            _ => Err(Error::WrongDeclaration),
        }
    }
}

pub enum PairedDeclaration<'lowered, S: Surface> {
    Record(DeclarationPair<'lowered, RecordDef, RecordDecl<S>>),
    Enum(DeclarationPair<'lowered, EnumDef, EnumDecl<S>>),
    Function(DeclarationPair<'lowered, FunctionDef, FunctionDecl<S>>),
    Class(DeclarationPair<'lowered, ClassDef, ClassDecl<S>>),
    Callback(DeclarationPair<'lowered, TraitDef, CallbackDecl<S>>),
    Stream(DeclarationPair<'lowered, StreamDef, StreamDecl<S>>),
    Constant(DeclarationPair<'lowered, ConstantDef, ConstantDecl<S>>),
}

pub struct DeclarationPair<'lowered, Source, Binding> {
    source: &'lowered Source,
    binding: &'lowered Binding,
}

impl<'lowered, Source, Binding> DeclarationPair<'lowered, Source, Binding> {
    pub fn new(source: &'lowered Source, binding: &'lowered Binding) -> Self {
        Self { source, binding }
    }

    pub fn source(&self) -> &'lowered Source {
        self.source
    }

    pub fn binding(&self) -> &'lowered Binding {
        self.binding
    }
}
