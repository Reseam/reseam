// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The text of a patch's `StringRef`s, from the string tables the Reseam
//! Gradle plugin compiles into the patch jar.

use std::collections::{BTreeMap, HashMap};

use jni::objects::{JByteArray, JClass, JObject, JValue, JValueOwned};
use jni::{Env, jni_sig, jni_str};
use serde::Deserialize;
use serde::de::IgnoredAny;

use super::callback::load_class;
use super::jni_values::{list_strings, object, string};
use super::jvm::jvm_err;
use crate::error::Result;

const TABLE_FILES: &str = "app.reseam.patch.settings.StringTableFiles";
const SOURCE_LOCALE: &str = "default";
const STRING_REF: &str = "()Lapp/reseam/patch/settings/StringRef;";

/// One string in every locale of its table: the source text, and the
/// translation of each locale that translates it.
pub(in crate::bridge) struct Text {
    pub(in crate::bridge) source: String,
    /// By BCP 47 tag.
    pub(in crate::bridge) translations: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TableValue {
    Text(String),
    Plural(IgnoredAny),
}

struct Table {
    /// The strings of each locale, `default` holding the source.
    locales: Vec<(String, BTreeMap<String, TableValue>)>,
}

/// The string tables a bundle's patches name, each read once.
pub(in crate::bridge) struct StringTables<'a> {
    files: JClass<'a>,
    tables: HashMap<String, Table>,
}

impl<'a> StringTables<'a> {
    pub(in crate::bridge) fn new(env: &mut Env<'a>, loader: &JObject<'_>) -> Result<Self> {
        let class = load_class(env, loader, TABLE_FILES)?;
        Ok(Self {
            files: JClass::cast_local(env, class)?,
            tables: HashMap::new(),
        })
    }

    /// The text of the `StringRef` that `getter` of `target` returns, or
    /// `None` when it returns null.
    pub(in crate::bridge) fn optional(
        &mut self,
        env: &mut Env<'_>,
        target: &JObject<'_>,
        getter: &str,
    ) -> Result<Option<Text>> {
        let reference = object(env, target, getter, STRING_REF)?;
        if reference.is_null() {
            return Ok(None);
        }
        self.text(env, &reference)
            .map(Some)
            .map_err(|error| jvm_err(format!("{getter}: {error}")))
    }

    /// The text `reference`, a `StringRef`, names.
    fn text(&mut self, env: &mut Env<'_>, reference: &JObject<'_>) -> Result<Text> {
        let table = object(
            env,
            reference,
            "getTable",
            "()Lapp/reseam/patch/settings/StringTable;",
        )?;
        let id = string(env, &table, "getId")?;
        let name = string(env, reference, "getName")?;
        if !self.tables.contains_key(&id) {
            let read = self.read(env, &table, &id)?;
            self.tables.insert(id.clone(), read);
        }
        let mut source = None;
        let mut translations = BTreeMap::new();
        for (locale, strings) in &self.tables[&id].locales {
            let text = match strings.get(&name) {
                None => continue,
                Some(TableValue::Text(text)) => text.clone(),
                Some(TableValue::Plural(_)) => {
                    return Err(jvm_err(format!(
                        "string '{name}' of {id} is a plural; patch text must be plain"
                    )));
                }
            };
            if locale == SOURCE_LOCALE {
                source = Some(text);
            } else {
                translations.insert(locale.clone(), text);
            }
        }
        let source =
            source.ok_or_else(|| jvm_err(format!("{id} has no source text for '{name}'")))?;
        Ok(Text {
            source,
            translations,
        })
    }

    fn read(&self, env: &mut Env<'_>, table: &JObject<'_>, id: &str) -> Result<Table> {
        let list = env
            .call_static_method(
                &self.files,
                jni_str!("locales"),
                jni_sig!("(Lapp/reseam/patch/settings/StringTable;)Ljava/util/List;"),
                &[JValue::Object(table)],
            )
            .and_then(JValueOwned::l)
            .map_err(|error| jvm_err(format!("{TABLE_FILES}.locales({id}): {error}")))?;
        let locales = list_strings(env, &list, "locales")?
            .into_iter()
            .map(|locale| {
                let json = self.json(env, table, id, &locale)?;
                let strings = serde_json::from_slice(&json)
                    .map_err(|error| jvm_err(format!("{id}/{locale}.json: {error}")))?;
                Ok((locale, strings))
            })
            .collect::<Result<_>>()?;
        Ok(Table { locales })
    }

    fn json(
        &self,
        env: &mut Env<'_>,
        table: &JObject<'_>,
        id: &str,
        locale: &str,
    ) -> Result<Vec<u8>> {
        let locale_j = env.new_string(locale)?;
        let bytes = env
            .call_static_method(
                &self.files,
                jni_str!("json"),
                jni_sig!("(Lapp/reseam/patch/settings/StringTable;Ljava/lang/String;)[B"),
                &[JValue::Object(table), JValue::Object(&locale_j)],
            )
            .and_then(JValueOwned::l)
            .map_err(|error| jvm_err(format!("{TABLE_FILES}.json({id}, {locale}): {error}")))?;
        let bytes = JByteArray::cast_local(env, bytes)?;
        Ok(env.convert_byte_array(bytes)?)
    }
}
