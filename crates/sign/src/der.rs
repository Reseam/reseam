// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Minimal DER encoder for the subset needed by self-signed ECDSA certs.

const TAG_INTEGER: u8 = 0x02;
const TAG_BIT_STRING: u8 = 0x03;
const TAG_OID: u8 = 0x06;
const TAG_UTF8_STRING: u8 = 0x0C;
const TAG_SEQUENCE: u8 = 0x30;
const TAG_SET: u8 = 0x31;

const OID_EC_PUBLIC_KEY: &[u8] = &[0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x02, 0x01];
const OID_PRIME256V1: &[u8] = &[0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x03, 0x01, 0x07];
const OID_ECDSA_WITH_SHA256: &[u8] = &[0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x04, 0x03, 0x02];
const OID_COMMON_NAME: &[u8] = &[0x55, 0x04, 0x03];
const OID_ORG_NAME: &[u8] = &[0x55, 0x04, 0x0A];

fn encode_length(len: usize) -> Vec<u8> {
    if len < 0x80 {
        vec![len as u8]
    } else if len < 0x100 {
        vec![0x81, len as u8]
    } else if len < 0x10000 {
        vec![0x82, (len >> 8) as u8, len as u8]
    } else {
        vec![0x83, (len >> 16) as u8, (len >> 8) as u8, len as u8]
    }
}

fn wrap(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    out.extend_from_slice(&encode_length(content.len()));
    out.extend_from_slice(content);
    out
}

pub fn sequence(items: &[&[u8]]) -> Vec<u8> {
    let content: Vec<u8> = items.iter().flat_map(|i| i.iter().copied()).collect();
    wrap(TAG_SEQUENCE, &content)
}

fn set(items: &[&[u8]]) -> Vec<u8> {
    let content: Vec<u8> = items.iter().flat_map(|i| i.iter().copied()).collect();
    wrap(TAG_SET, &content)
}

pub fn integer(value: &[u8]) -> Vec<u8> {
    if value.is_empty() {
        return wrap(TAG_INTEGER, &[0x00]);
    }
    if value[0] & 0x80 != 0 {
        let mut padded = vec![0x00];
        padded.extend_from_slice(value);
        wrap(TAG_INTEGER, &padded)
    } else {
        wrap(TAG_INTEGER, value)
    }
}

pub fn integer_u64(v: u64) -> Vec<u8> {
    let bytes = v.to_be_bytes();
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(7);
    integer(&bytes[start..])
}

pub fn oid(value: &[u8]) -> Vec<u8> {
    wrap(TAG_OID, value)
}

pub fn utf8_string(s: &str) -> Vec<u8> {
    wrap(TAG_UTF8_STRING, s.as_bytes())
}

pub fn bit_string(content: &[u8]) -> Vec<u8> {
    let mut inner = vec![0x00];
    inner.extend_from_slice(content);
    wrap(TAG_BIT_STRING, &inner)
}

pub fn explicit_tag(tag_num: u8, content: &[u8]) -> Vec<u8> {
    wrap(0xA0 | tag_num, content)
}

pub fn ecdsa_sha256_algorithm() -> Vec<u8> {
    sequence(&[&oid(OID_ECDSA_WITH_SHA256)])
}

pub fn ec_p256_algorithm() -> Vec<u8> {
    sequence(&[&oid(OID_EC_PUBLIC_KEY), &oid(OID_PRIME256V1)])
}

pub fn name(cn: &str, org: &str) -> Vec<u8> {
    let cn_attr = set(&[&sequence(&[&oid(OID_COMMON_NAME), &utf8_string(cn)])]);
    let org_attr = set(&[&sequence(&[&oid(OID_ORG_NAME), &utf8_string(org)])]);
    sequence(&[&cn_attr, &org_attr])
}

/// Dates as UTCTime: "YYMMDDHHMMSSZ".
pub fn validity(not_before: &str, not_after: &str) -> Vec<u8> {
    let nb = wrap(0x17, not_before.as_bytes());
    let na = wrap(0x17, not_after.as_bytes());
    sequence(&[&nb, &na])
}

/// `public_key` should be the uncompressed EC point (65 bytes: 0x04 || x || y).
pub fn ec_subject_public_key_info(public_key: &[u8]) -> Vec<u8> {
    let algo = ec_p256_algorithm();
    let pk_bits = bit_string(public_key);
    sequence(&[&algo, &pk_bits])
}
