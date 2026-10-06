// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{
    AccessFlags, ClassData, ClassDataCursor, ClassSkeleton, DexFile, EncodedField, EncodedMethod,
    Fingerprint, InstructionPattern, MemberCounts, MethodHeader, MethodHit, MethodIdx, RefQuery,
    Result, count_instructions, decode_one_method, read_class_skeleton_at, read_u16, read_u32,
};

/// A method's identity and frame shape, read without decoding its code.
#[derive(Debug, Clone, Copy)]
pub struct MethodSummary {
    pub method: MethodIdx,
    pub access_flags: AccessFlags,
    pub registers_size: u16,
    pub ins_size: u16,
    pub outs_size: u16,
    pub has_code: bool,
    pub instruction_count: u32,
}

impl DexFile {
    /// The member lists of a still-deferred class, read from the raw buffer
    /// without touching any code. `None` for materialized classes and classes
    /// without class data.
    pub fn class_skeleton(&self, class_idx: usize) -> Result<Option<ClassSkeleton>> {
        let Some(offset) = self.raw_class_data_offset(class_idx) else {
            return Ok(None);
        };
        let buf = self.raw_bytes(offset)?;
        Ok(Some(read_class_skeleton_at(
            buf,
            offset as usize,
            self.parse_options,
        )?))
    }

    /// Summarizes a deferred method from its skeleton entry: the 16-byte code
    /// item header plus an opcode-length walk for the instruction count.
    pub fn summarize_method(&self, header: &MethodHeader) -> Result<MethodSummary> {
        let mut summary = MethodSummary {
            method: header.method,
            access_flags: header.access_flags,
            registers_size: 0,
            ins_size: 0,
            outs_size: 0,
            has_code: header.code_off != 0,
            instruction_count: 0,
        };
        if header.code_off != 0 {
            let buf = self.raw_bytes(header.code_off)?;
            let base = header.code_off as usize;
            summary.registers_size = read_u16(buf, base)?;
            summary.ins_size = read_u16(buf, base + 2)?;
            summary.outs_size = read_u16(buf, base + 4)?;
            let insns_size = read_u32(buf, base + 12)? as usize;
            summary.instruction_count = count_instructions(buf, base + 16, insns_size)?;
        }
        Ok(summary)
    }

    /// Summarizes one method: from IR when its class is materialized, else via
    /// [`Self::class_skeleton`] and [`Self::summarize_method`].
    pub fn method_summary(
        &self,
        class_idx: usize,
        method_pos: usize,
        kind: crate::types::class::MethodKind,
    ) -> Result<Option<MethodSummary>> {
        if let Some(data) = self.resident_class_data(class_idx) {
            let list = if kind == crate::types::class::MethodKind::Virtual {
                &data.virtual_methods
            } else {
                &data.direct_methods
            };
            return Ok(list.get(method_pos).map(summarize_resident));
        }
        let Some(skeleton) = self.class_skeleton(class_idx)? else {
            return Ok(None);
        };
        skeleton
            .method(method_pos, kind)
            .map(|header| self.summarize_method(header))
            .transpose()
    }
}

pub fn summarize_resident(m: &EncodedMethod) -> MethodSummary {
    let code = m.code.as_ref();
    MethodSummary {
        method: m.method,
        access_flags: m.access_flags,
        registers_size: code.map_or(0, |c| c.registers_size),
        ins_size: code.map_or(0, |c| c.ins_size),
        outs_size: code.map_or(0, |c| c.outs_size),
        has_code: code.is_some(),
        instruction_count: code.map_or(0, |c| c.instructions.len() as u32),
    }
}

impl DexFile {
    /// Class member counts without materializing the class. For a deferred
    /// class this reads only the four `class_data` header LEBs — no field,
    /// method, or code decoding.
    pub fn class_member_counts(&self, class_idx: usize) -> Result<Option<MemberCounts>> {
        if class_idx >= self.classes.len() {
            return Ok(None);
        }
        if let Some(data) = self.resident_class_data(class_idx) {
            return Ok(Some(MemberCounts {
                direct_methods: data.direct_methods.len() as u32,
                virtual_methods: data.virtual_methods.len() as u32,
                static_fields: data.static_fields.len() as u32,
                instance_fields: data.instance_fields.len() as u32,
            }));
        }
        let Some(offset) = self.raw_class_data_offset(class_idx) else {
            return Ok(Some(MemberCounts::default()));
        };
        let buf = self.raw_bytes(offset)?;
        let opts = self.parse_options;
        Ok(Some(
            ClassDataCursor::new(buf, offset as usize, opts)?.counts,
        ))
    }

    /// Class fields `(static, instance)` without materializing the class. For a
    /// deferred class this decodes only the encoded-field entries, never methods
    /// or code.
    pub fn decode_class_fields(
        &self,
        class_idx: usize,
    ) -> Result<Option<(Vec<EncodedField>, Vec<EncodedField>)>> {
        if class_idx >= self.classes.len() {
            return Ok(None);
        }
        if let Some(data) = self.resident_class_data(class_idx) {
            return Ok(Some((
                data.static_fields.clone(),
                data.instance_fields.clone(),
            )));
        }
        let Some(offset) = self.raw_class_data_offset(class_idx) else {
            return Ok(Some((Vec::new(), Vec::new())));
        };
        let buf = self.raw_bytes(offset)?;
        let opts = self.parse_options;
        let mut cursor = ClassDataCursor::new(buf, offset as usize, opts)?;
        let mut static_fields = Vec::new();
        let mut instance_fields = Vec::new();
        cursor.fields(cursor.counts.static_fields, |field| {
            static_fields.push(field);
        })?;
        cursor.fields(cursor.counts.instance_fields, |field| {
            instance_fields.push(field);
        })?;
        Ok(Some((static_fields, instance_fields)))
    }

    pub(crate) fn raw_class_data_offset(&self, class_idx: usize) -> Option<u32> {
        self.classes
            .raw_def(class_idx)
            .map(|def| def.class_data_off)
            .filter(|&off| off != 0)
    }

    pub(super) fn resident_class_data(&self, class_idx: usize) -> Option<&ClassData> {
        self.classes.resident(class_idx)?.class_data.as_deref()
    }

    pub(crate) fn raw_bytes(&self, offset: u32) -> Result<&[u8]> {
        Ok(self
            .raw
            .as_ref()
            .ok_or_else(|| crate::error::invalid_offset("class data", offset, 0))?
            .as_bytes())
    }

    /// Decodes one method at a position without persisting its class.
    ///
    /// If the class is already materialized, clones from its IR; otherwise
    /// decodes just that one method from the raw buffer, leaving the class
    /// deferred. Read-only inspection uses this so reading a method never
    /// permanently materializes the rest of its class.
    pub fn decode_method_at(
        &self,
        class_idx: usize,
        method_pos: usize,
        kind: crate::types::class::MethodKind,
    ) -> Result<Option<EncodedMethod>> {
        if let Some(data) = self.resident_class_data(class_idx) {
            let list = if kind == crate::types::class::MethodKind::Virtual {
                &data.virtual_methods
            } else {
                &data.direct_methods
            };
            return Ok(list.get(method_pos).cloned());
        }

        let Some(offset) = self.raw_class_data_offset(class_idx) else {
            return Ok(None);
        };
        let source = self
            .raw
            .as_ref()
            .ok_or_else(|| crate::error::invalid_offset("class data", offset, 0))?;
        decode_one_method(source, offset, method_pos, kind, self.parse_options)
    }
}

impl DexFile {
    pub fn find_method_by_name(&self, name: &str) -> Result<Option<MethodHit>> {
        let Some(name) = self.find_string_idx(name) else {
            return Ok(None);
        };
        self.scan_methods_find(&RefQuery::default(), |view| {
            Ok((self.method_id(view.method).name == name).then(|| view.hit()))
        })
    }

    /// Finds every method whose body loads all of the given string constants.
    ///
    /// The string set is resolved to indices once per DEX; a DEX missing any of
    /// them is rejected before a single method is scanned.
    pub fn find_methods_by_strings(&self, strings: &[&str]) -> Result<Vec<MethodHit>> {
        self.find_methods_by_fingerprint(&Fingerprint {
            strings: Some(strings.iter().map(|value| (*value).to_owned()).collect()),
            ..Fingerprint::default()
        })
        .map(|hits| hits.into_iter().map(|hit| hit.method).collect())
    }

    pub fn find_methods_with_opcodes(
        &self,
        opcodes: &[InstructionPattern],
    ) -> Result<Vec<MethodHit>> {
        self.find_methods_by_fingerprint(&Fingerprint {
            opcodes: Some(opcodes.to_vec()),
            ..Fingerprint::default()
        })
        .map(|hits| hits.into_iter().map(|hit| hit.method).collect())
    }
}
