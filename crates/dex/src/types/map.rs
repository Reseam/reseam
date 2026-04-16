// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

#[derive(Debug, Clone)]
pub struct MapItem {
    pub type_code: u16,
    pub size: u32,
    pub offset: u32,
}

pub const TYPE_HEADER_ITEM: u16 = 0x0000;
pub const TYPE_STRING_ID_ITEM: u16 = 0x0001;
pub const TYPE_TYPE_ID_ITEM: u16 = 0x0002;
pub const TYPE_PROTO_ID_ITEM: u16 = 0x0003;
pub const TYPE_FIELD_ID_ITEM: u16 = 0x0004;
pub const TYPE_METHOD_ID_ITEM: u16 = 0x0005;
pub const TYPE_CLASS_DEF_ITEM: u16 = 0x0006;
pub const TYPE_CALL_SITE_ID_ITEM: u16 = 0x0007;
pub const TYPE_METHOD_HANDLE_ITEM: u16 = 0x0008;
pub const TYPE_MAP_LIST: u16 = 0x1000;
pub const TYPE_TYPE_LIST: u16 = 0x1001;
pub const TYPE_ANNOTATION_SET_REF_LIST: u16 = 0x1002;
pub const TYPE_ANNOTATION_SET_ITEM: u16 = 0x1003;
pub const TYPE_CLASS_DATA_ITEM: u16 = 0x2000;
pub const TYPE_CODE_ITEM: u16 = 0x2001;
pub const TYPE_STRING_DATA_ITEM: u16 = 0x2002;
pub const TYPE_DEBUG_INFO_ITEM: u16 = 0x2003;
pub const TYPE_ANNOTATION_ITEM: u16 = 0x2004;
pub const TYPE_ENCODED_ARRAY_ITEM: u16 = 0x2005;
pub const TYPE_ANNOTATIONS_DIRECTORY_ITEM: u16 = 0x2006;
pub const TYPE_HIDDENAPI_CLASS_DATA_ITEM: u16 = 0xF000;
