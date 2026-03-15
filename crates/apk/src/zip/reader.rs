use crate::error::Result;
use std::io::{Read, Seek};

/// Wrapper around ZipArchive providing APK-oriented access.
pub struct ApkReader<R: Read + Seek> {
    archive: zip::ZipArchive<R>,
}

impl<R: Read + Seek> ApkReader<R> {
    /// Open a ZIP archive for reading.
    pub fn new(reader: R) -> Result<Self> {
        let archive = zip::ZipArchive::new(reader)?;
        Ok(Self { archive })
    }

    /// Number of entries in the archive.
    pub fn len(&self) -> usize {
        self.archive.len()
    }

    pub fn is_empty(&self) -> bool {
        self.archive.is_empty()
    }

    /// List all entry names in the archive.
    pub fn entry_names(&mut self) -> Vec<String> {
        (0..self.archive.len())
            .filter_map(|i| {
                self.archive
                    .by_index_raw(i)
                    .ok()
                    .map(|e| e.name().to_string())
            })
            .collect()
    }

    /// Read a single entry's decompressed contents by name.
    pub fn read_entry(&mut self, name: &str) -> Result<Vec<u8>> {
        let mut entry = self.archive.by_name(name)?;
        let mut buf = Vec::with_capacity(entry.size() as usize);
        std::io::Read::read_to_end(&mut entry, &mut buf)?;
        Ok(buf)
    }

    /// Check if an entry exists.
    pub fn contains(&self, name: &str) -> bool {
        // ZipArchive doesn't have a contains method, but we can check via index_for_name
        self.archive.index_for_name(name).is_some()
    }

    /// Get the list of DEX entry names (classes.dex, classes2.dex, ...) in sorted order.
    pub fn dex_entry_names(&mut self) -> Vec<String> {
        let mut names: Vec<String> = self
            .entry_names()
            .into_iter()
            .filter(|name| {
                name.ends_with(".dex")
                    && (name == "classes.dex"
                        || (name.starts_with("classes") && name.ends_with(".dex")))
            })
            .collect();
        names.sort_by_key(|n| dex_sort_key(n));
        names
    }

    /// Read all DEX entries as raw byte buffers, sorted by index.
    pub fn read_all_dex(&mut self) -> Result<Vec<(String, Vec<u8>)>> {
        let names = self.dex_entry_names();
        let mut result = Vec::with_capacity(names.len());
        for name in names {
            let buf = self.read_entry(&name)?;
            result.push((name, buf));
        }
        Ok(result)
    }

    /// Read AndroidManifest.xml as raw bytes (binary XML format).
    pub fn read_manifest(&mut self) -> Result<Vec<u8>> {
        self.read_entry("AndroidManifest.xml")
    }

    /// Consume self and return the inner ZipArchive (needed by writer for pass-through).
    pub fn into_archive(self) -> zip::ZipArchive<R> {
        self.archive
    }

    /// Borrow the inner ZipArchive.
    pub fn archive(&self) -> &zip::ZipArchive<R> {
        &self.archive
    }

    /// Mutably borrow the inner ZipArchive.
    pub fn archive_mut(&mut self) -> &mut zip::ZipArchive<R> {
        &mut self.archive
    }
}

/// Sort key for DEX file names: classes.dex=1, classes2.dex=2, etc.
fn dex_sort_key(name: &str) -> u32 {
    if name == "classes.dex" {
        return 1;
    }
    let stripped = name.strip_prefix("classes").unwrap_or("0");
    let num_str = stripped.strip_suffix(".dex").unwrap_or("0");
    num_str.parse().unwrap_or(0)
}
