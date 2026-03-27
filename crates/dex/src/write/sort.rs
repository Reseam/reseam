use crate::types::annotation::{AnnotationItem, AnnotationsDirectory};
use crate::types::class::{ClassData, ClassDef, EncodedMethod};
use crate::types::code::CodeItem;
use crate::types::debug::{DebugBytecode, DebugInfo};
use crate::types::encoded_value::EncodedValue;
use crate::types::instruction::Instruction;
use crate::types::method_handle::{CallSiteItem, MethodHandle, MethodHandleMember};
use crate::types::{FieldId, FieldIdx, MethodId, MethodIdx, Prototype, ProtoIdx, StringIdx, TypeIdx};
use crate::file::DexFile;
use crate::util::sort::dex_string_compare;

/// Sort all index tables and remap all references in place.
/// Prepares the DexFile for writing with correctly sorted tables.
pub fn sort_in_place(dex: &mut DexFile) {
    dex.raw = None;
    dex.lazy_class_data_offsets = None;

    let mut string_order: Vec<u32> = (0..dex.strings.len() as u32).collect();
    string_order.sort_by(|&a, &b| {
        dex_string_compare(
            dex.strings[a as usize].as_str(),
            dex.strings[b as usize].as_str(),
        )
    });

    let string_remap = build_remap(&string_order);

    let mut type_order: Vec<u32> = (0..dex.types.len() as u32).collect();
    type_order.sort_by_key(|&i| string_remap[dex.types[i as usize].0 as usize]);

    let type_remap = build_remap(&type_order);

    let mut proto_order: Vec<u16> = (0..dex.prototypes.len() as u16).collect();
    proto_order.sort_by(|&a, &b| {
        let pa = &dex.prototypes[a as usize];
        let pb = &dex.prototypes[b as usize];
        let ra = type_remap[pa.return_type.0 as usize];
        let rb = type_remap[pb.return_type.0 as usize];
        ra.cmp(&rb).then_with(|| {
            let params_a: Vec<u32> = pa
                .parameters
                .iter()
                .map(|t| type_remap[t.0 as usize])
                .collect();
            let params_b: Vec<u32> = pb
                .parameters
                .iter()
                .map(|t| type_remap[t.0 as usize])
                .collect();
            params_a.cmp(&params_b)
        })
    });

    let proto_remap = build_remap_u16(&proto_order);

    let mut field_order: Vec<u32> = (0..dex.fields.len() as u32).collect();
    field_order.sort_by(|&a, &b| {
        let fa = &dex.fields[a as usize];
        let fb = &dex.fields[b as usize];
        type_remap[fa.class.0 as usize]
            .cmp(&type_remap[fb.class.0 as usize])
            .then_with(|| string_remap[fa.name.0 as usize].cmp(&string_remap[fb.name.0 as usize]))
            .then_with(|| type_remap[fa.type_.0 as usize].cmp(&type_remap[fb.type_.0 as usize]))
    });

    let field_remap = build_remap(&field_order);

    let mut method_order: Vec<u32> = (0..dex.methods.len() as u32).collect();
    method_order.sort_by(|&a, &b| {
        let ma = &dex.methods[a as usize];
        let mb = &dex.methods[b as usize];
        type_remap[ma.class.0 as usize]
            .cmp(&type_remap[mb.class.0 as usize])
            .then_with(|| string_remap[ma.name.0 as usize].cmp(&string_remap[mb.name.0 as usize]))
            .then_with(|| proto_remap[ma.proto.0 as usize].cmp(&proto_remap[mb.proto.0 as usize]))
    });

    let method_remap = build_remap(&method_order);

    let already_sorted = is_identity(&string_remap)
        && is_identity(&type_remap)
        && is_identity_u16(&proto_remap)
        && is_identity(&field_remap)
        && is_identity(&method_remap);

    if already_sorted {
        return;
    }

    let remap = Remap {
        string: &string_remap,
        type_: &type_remap,
        proto: &proto_remap,
        field: &field_remap,
        method: &method_remap,
    };

    let new_strings: Vec<_> = string_order
        .iter()
        .map(|&i| dex.strings[i as usize].clone())
        .collect();

    let new_types: Vec<_> = type_order
        .iter()
        .map(|&i| StringIdx(remap.string[dex.types[i as usize].0 as usize]))
        .collect();

    let new_prototypes: Vec<_> = proto_order
        .iter()
        .map(|&i| {
            let p = &dex.prototypes[i as usize];
            Prototype {
                shorty: remap.remap_string(p.shorty),
                return_type: remap.remap_type(p.return_type),
                parameters: p.parameters.iter().map(|t| remap.remap_type(*t)).collect(),
            }
        })
        .collect();

    let new_fields: Vec<_> = field_order
        .iter()
        .map(|&i| {
            let f = &dex.fields[i as usize];
            FieldId {
                class: remap.remap_type(f.class),
                type_: remap.remap_type(f.type_),
                name: remap.remap_string(f.name),
            }
        })
        .collect();

    let new_methods: Vec<_> = method_order
        .iter()
        .map(|&i| {
            let m = &dex.methods[i as usize];
            MethodId {
                class: remap.remap_type(m.class),
                proto: remap.remap_proto(m.proto),
                name: remap.remap_string(m.name),
            }
        })
        .collect();

    dex.strings = new_strings;
    dex.types = new_types;
    dex.prototypes = new_prototypes;
    dex.fields = new_fields;
    dex.methods = new_methods;

    for class in &mut dex.classes {
        remap.remap_class(class);
    }

    for cs in &mut dex.call_sites {
        remap.remap_call_site(cs);
    }

    for mh in &mut dex.method_handles {
        remap.remap_method_handle(mh);
    }

    dex.classes.sort_by_key(|c| c.class_type.0);

    fixup_instructions(dex);

    dex.build_lookups();
}

pub(crate) struct Remap<'a> {
    pub(crate) string: &'a [u32],
    pub(crate) type_: &'a [u32],
    pub(crate) proto: &'a [u16],
    pub(crate) field: &'a [u32],
    pub(crate) method: &'a [u32],
}

impl<'a> Remap<'a> {
    pub(crate) fn remap_string(&self, idx: StringIdx) -> StringIdx {
        StringIdx(self.string[idx.0 as usize])
    }

    pub(crate) fn remap_type(&self, idx: TypeIdx) -> TypeIdx {
        TypeIdx(self.type_[idx.0 as usize])
    }

    pub(crate) fn remap_proto(&self, idx: ProtoIdx) -> ProtoIdx {
        ProtoIdx(self.proto[idx.0 as usize])
    }

    pub(crate) fn remap_field(&self, idx: FieldIdx) -> FieldIdx {
        FieldIdx(self.field[idx.0 as usize])
    }

    pub(crate) fn remap_method(&self, idx: MethodIdx) -> MethodIdx {
        MethodIdx(self.method[idx.0 as usize])
    }

    pub(crate) fn remap_opt_string(&self, idx: Option<StringIdx>) -> Option<StringIdx> {
        idx.map(|i| self.remap_string(i))
    }

    pub(crate) fn remap_opt_type(&self, idx: Option<TypeIdx>) -> Option<TypeIdx> {
        idx.map(|i| self.remap_type(i))
    }

    pub(crate) fn remap_class(&self, class: &mut ClassDef) {
        class.class_type = self.remap_type(class.class_type);
        class.superclass = self.remap_opt_type(class.superclass);
        class.interfaces = class
            .interfaces
            .iter()
            .map(|t| self.remap_type(*t))
            .collect();
        class.source_file = self.remap_opt_string(class.source_file);

        if let Some(ref mut ann) = class.annotations {
            self.remap_annotations_dir(ann);
        }

        if let Some(ref mut data) = class.class_data {
            self.remap_class_data(data);
        }

        for sv in &mut class.static_values {
            self.remap_encoded_value(sv);
        }
    }

    fn remap_class_data(&self, data: &mut ClassData) {
        for f in &mut data.static_fields {
            f.field = self.remap_field(f.field);
        }
        for f in &mut data.instance_fields {
            f.field = self.remap_field(f.field);
        }
        // Re-sort fields by new index (delta encoding requires sorted order)
        data.static_fields.sort_by_key(|f| f.field.0);
        data.instance_fields.sort_by_key(|f| f.field.0);

        for m in &mut data.direct_methods {
            self.remap_encoded_method(m);
        }
        for m in &mut data.virtual_methods {
            self.remap_encoded_method(m);
        }
        // Re-sort methods by new index
        data.direct_methods.sort_by_key(|m| m.method.0);
        data.virtual_methods.sort_by_key(|m| m.method.0);
    }

    fn remap_encoded_method(&self, m: &mut EncodedMethod) {
        m.method = self.remap_method(m.method);
        if let Some(ref mut code) = m.code {
            self.remap_code(code);
        }
    }

    fn remap_code(&self, code: &mut CodeItem) {
        for insn in &mut code.instructions {
            self.remap_instruction(insn);
        }
        for handler in &mut code.catch_handlers {
            for tc in &mut handler.typed_catches {
                tc.exception_type = self.remap_type(tc.exception_type);
            }
        }
        if let Some(ref mut debug) = code.debug_info {
            self.remap_debug(debug);
        }
    }

    fn remap_debug(&self, debug: &mut DebugInfo) {
        for name in &mut debug.parameter_names {
            *name = self.remap_opt_string(*name);
        }
        for bc in &mut debug.bytecodes {
            match bc {
                DebugBytecode::StartLocal { name, type_, .. } => {
                    *name = self.remap_opt_string(*name);
                    *type_ = self.remap_opt_type(*type_);
                }
                DebugBytecode::StartLocalExtended {
                    name,
                    type_,
                    signature,
                    ..
                } => {
                    *name = self.remap_opt_string(*name);
                    *type_ = self.remap_opt_type(*type_);
                    *signature = self.remap_opt_string(*signature);
                }
                DebugBytecode::SetFile { name } => {
                    *name = self.remap_opt_string(*name);
                }
                _ => {}
            }
        }
    }

    fn remap_annotations_dir(&self, dir: &mut AnnotationsDirectory) {
        for item in &mut dir.class_annotations {
            self.remap_annotation_item(item);
        }
        dir.class_annotations.sort_by_key(|item| item.type_.0);

        for (field_idx, items) in &mut dir.field_annotations {
            *field_idx = self.remap_field(*field_idx);
            for item in items.iter_mut() {
                self.remap_annotation_item(item);
            }
            items.sort_by_key(|item| item.type_.0);
        }
        dir.field_annotations.sort_by_key(|(idx, _)| idx.0);

        for (method_idx, items) in &mut dir.method_annotations {
            *method_idx = self.remap_method(*method_idx);
            for item in items.iter_mut() {
                self.remap_annotation_item(item);
            }
            items.sort_by_key(|item| item.type_.0);
        }
        dir.method_annotations.sort_by_key(|(idx, _)| idx.0);

        for (method_idx, param_items) in &mut dir.parameter_annotations {
            *method_idx = self.remap_method(*method_idx);
            for items in param_items.iter_mut() {
                for item in items.iter_mut() {
                    self.remap_annotation_item(item);
                }
                items.sort_by_key(|item| item.type_.0);
            }
        }
        dir.parameter_annotations.sort_by_key(|(idx, _)| idx.0);
    }

    fn remap_annotation_item(&self, item: &mut AnnotationItem) {
        item.type_ = self.remap_type(item.type_);
        for elem in &mut item.elements {
            elem.name = self.remap_string(elem.name);
            self.remap_encoded_value(&mut elem.value);
        }
        item.elements.sort_by_key(|e| e.name.0);
    }

    fn remap_encoded_value(&self, v: &mut EncodedValue) {
        match v {
            EncodedValue::String(idx) => *idx = self.remap_string(*idx),
            EncodedValue::Type(idx) => *idx = self.remap_type(*idx),
            EncodedValue::Field(idx) => *idx = self.remap_field(*idx),
            EncodedValue::Method(idx) => *idx = self.remap_method(*idx),
            EncodedValue::Enum(idx) => *idx = self.remap_field(*idx),
            EncodedValue::MethodType(idx) => *idx = self.remap_proto(*idx),
            EncodedValue::Array(items) => {
                for item in items {
                    self.remap_encoded_value(item);
                }
            }
            EncodedValue::Annotation(ann) => {
                ann.type_ = self.remap_type(ann.type_);
                for elem in &mut ann.elements {
                    elem.name = self.remap_string(elem.name);
                    self.remap_encoded_value(&mut elem.value);
                }
                ann.elements.sort_by_key(|e| e.name.0);
            }
            _ => {}
        }
    }

    fn remap_instruction(&self, insn: &mut crate::types::instruction::Instruction) {
        use crate::types::instruction::Instruction;
        match insn {
            Instruction::ConstString { string, .. }
            | Instruction::ConstStringJumbo { string, .. } => {
                *string = self.remap_string(*string);
            }
            Instruction::ConstClass { type_, .. }
            | Instruction::CheckCast { type_, .. }
            | Instruction::NewInstance { type_, .. } => {
                *type_ = self.remap_type(*type_);
            }
            Instruction::InstanceOf { type_, .. } | Instruction::NewArray { type_, .. } => {
                *type_ = self.remap_type(*type_);
            }
            Instruction::FilledNewArray { type_, .. }
            | Instruction::FilledNewArrayRange { type_, .. } => {
                *type_ = self.remap_type(*type_);
            }
            Instruction::Iget { field, .. }
            | Instruction::IgetWide { field, .. }
            | Instruction::IgetObject { field, .. }
            | Instruction::IgetBoolean { field, .. }
            | Instruction::IgetByte { field, .. }
            | Instruction::IgetChar { field, .. }
            | Instruction::IgetShort { field, .. }
            | Instruction::Iput { field, .. }
            | Instruction::IputWide { field, .. }
            | Instruction::IputObject { field, .. }
            | Instruction::IputBoolean { field, .. }
            | Instruction::IputByte { field, .. }
            | Instruction::IputChar { field, .. }
            | Instruction::IputShort { field, .. }
            | Instruction::Sget { field, .. }
            | Instruction::SgetWide { field, .. }
            | Instruction::SgetObject { field, .. }
            | Instruction::SgetBoolean { field, .. }
            | Instruction::SgetByte { field, .. }
            | Instruction::SgetChar { field, .. }
            | Instruction::SgetShort { field, .. }
            | Instruction::Sput { field, .. }
            | Instruction::SputWide { field, .. }
            | Instruction::SputObject { field, .. }
            | Instruction::SputBoolean { field, .. }
            | Instruction::SputByte { field, .. }
            | Instruction::SputChar { field, .. }
            | Instruction::SputShort { field, .. } => {
                *field = self.remap_field(*field);
            }
            Instruction::InvokeVirtual { method, .. }
            | Instruction::InvokeSuper { method, .. }
            | Instruction::InvokeDirect { method, .. }
            | Instruction::InvokeStatic { method, .. }
            | Instruction::InvokeInterface { method, .. }
            | Instruction::InvokeVirtualRange { method, .. }
            | Instruction::InvokeSuperRange { method, .. }
            | Instruction::InvokeDirectRange { method, .. }
            | Instruction::InvokeStaticRange { method, .. }
            | Instruction::InvokeInterfaceRange { method, .. } => {
                *method = self.remap_method(*method);
            }
            Instruction::InvokePolymorphic { method, proto, .. }
            | Instruction::InvokePolymorphicRange { method, proto, .. } => {
                *method = self.remap_method(*method);
                *proto = self.remap_proto(*proto);
            }
            Instruction::ConstMethodType { proto, .. } => {
                *proto = self.remap_proto(*proto);
            }
            // InvokeCustom/InvokeCustomRange reference call_site indices, not remapped
            // ConstMethodHandle references method_handle indices, not remapped
            _ => {}
        }
    }

    pub(crate) fn remap_call_site(&self, cs: &mut CallSiteItem) {
        cs.method_name = self.remap_string(cs.method_name);
        cs.method_type = self.remap_proto(cs.method_type);
        for arg in &mut cs.extra_arguments {
            self.remap_encoded_value(arg);
        }
    }

    pub(crate) fn remap_method_handle(&self, mh: &mut MethodHandle) {
        match &mut mh.member {
            MethodHandleMember::Field(idx) => *idx = self.remap_field(*idx),
            MethodHandleMember::Method(idx) => *idx = self.remap_method(*idx),
        }
    }
}

/// Build old_idx → new_idx remap from a permutation array (new_idx → old_idx).
fn build_remap(order: &[u32]) -> Vec<u32> {
    let mut remap = vec![0u32; order.len()];
    for (new_idx, &old_idx) in order.iter().enumerate() {
        remap[old_idx as usize] = new_idx as u32;
    }
    remap
}

fn build_remap_u16(order: &[u16]) -> Vec<u16> {
    let mut remap = vec![0u16; order.len()];
    for (new_idx, &old_idx) in order.iter().enumerate() {
        remap[old_idx as usize] = new_idx as u16;
    }
    remap
}

fn is_identity(remap: &[u32]) -> bool {
    remap.iter().enumerate().all(|(i, &v)| v == i as u32)
}

fn is_identity_u16(remap: &[u16]) -> bool {
    remap.iter().enumerate().all(|(i, &v)| v == i as u16)
}

fn fixup_instructions(dex: &mut DexFile) {
    for class in &mut dex.classes {
        let data = match class.class_data.as_mut() {
            Some(d) => d,
            None => continue,
        };
        for method in data.direct_methods.iter_mut().chain(data.virtual_methods.iter_mut()) {
            let code = match method.code.as_mut() {
                Some(c) => c,
                None => continue,
            };
            let mut i = 0;
            while i < code.instructions.len() {
                match &code.instructions[i] {
                    Instruction::ConstString { dest, string } if string.0 > 0xFFFF => {
                        let promoted = Instruction::ConstStringJumbo {
                            dest: *dest,
                            string: *string,
                        };
                        code.replace_instruction(i, promoted);
                    }
                    _ => {}
                }
                i += 1;
            }
        }
    }
}
