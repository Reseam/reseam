use crate::error::Result;
use crate::zip::reader::ApkReader;
use stitch_dex::{DexError, MultiDexContainer, ParseOptions};
use std::io::{Read, Seek};

/// Extract and parse all DEX files from a single APK reader.
pub fn extract_dex<R: Read + Seek>(
    reader: &mut ApkReader<R>,
    opts: ParseOptions,
) -> Result<MultiDexContainer> {
    let dex_entries = reader.read_all_dex()?;
    let buffers: Vec<&[u8]> = dex_entries.iter().map(|(_, buf)| buf.as_slice()).collect();
    let container = MultiDexContainer::parse(&buffers, opts)?;
    Ok(container)
}

/// Extract DEX from multiple APK readers (base + splits) into a unified container.
///
/// All DEX from all APKs is merged into a single `MultiDexContainer`.
/// Config splits with no DEX are silently skipped.
pub fn extract_dex_unified<R: Read + Seek>(
    readers: &mut [&mut ApkReader<R>],
    opts: ParseOptions,
) -> Result<MultiDexContainer> {
    let mut all_buffers: Vec<Vec<u8>> = Vec::new();

    for reader in readers.iter_mut() {
        let dex_entries = reader.read_all_dex()?;
        for (_, buf) in dex_entries {
            all_buffers.push(buf);
        }
    }

    let refs: Vec<&[u8]> = all_buffers.iter().map(|b| b.as_slice()).collect();
    let container = MultiDexContainer::parse(&refs, opts)?;
    Ok(container)
}

/// Serialize all DEX files in a container to named entries for writing into an APK.
///
/// Returns entries named `classes.dex`, `classes2.dex`, `classes3.dex`, etc.
pub fn dex_to_entries(container: &mut MultiDexContainer) -> Result<Vec<(String, Vec<u8>)>> {
    let buffers = container.write_all()?;
    let mut entries = Vec::with_capacity(buffers.len());
    for (i, buf) in buffers.into_iter().enumerate() {
        let name = if i == 0 {
            "classes.dex".to_string()
        } else {
            format!("classes{}.dex", i + 1)
        };
        entries.push((name, buf));
    }
    Ok(entries)
}

/// Extracts and parses `classes*.dex` entries from an APK or ZIP byte slice.
///
/// Convenience function for loading DEX from raw APK bytes without constructing an `ApkReader`.
pub fn from_apk(apk_bytes: &[u8], opts: ParseOptions) -> stitch_dex::Result<MultiDexContainer> {
    use std::io::Cursor;

    let reader = Cursor::new(apk_bytes);
    let mut archive = ::zip::ZipArchive::new(reader)
        .map_err(|e| DexError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

    let mut dex_names: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            let name = archive.by_index(i).ok()?.name().to_string();
            if name.ends_with(".dex")
                && (name == "classes.dex"
                    || name.starts_with("classes") && name.ends_with(".dex"))
            {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    dex_names.sort_by_key(|a| dex_sort_key(a));

    let mut dex_files = Vec::with_capacity(dex_names.len());
    for name in &dex_names {
        let mut entry = archive
            .by_name(name)
            .map_err(|e| DexError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, e)))?;
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf).map_err(DexError::Io)?;
        dex_files.push(stitch_dex::parse(&buf, opts.clone())?);
    }

    let mut container = MultiDexContainer::new();
    for dex in dex_files {
        container.add_dex(dex);
    }
    Ok(container)
}

fn dex_sort_key(name: &str) -> u32 {
    if name == "classes.dex" {
        return 1;
    }
    let stripped = name.strip_prefix("classes").unwrap_or("0");
    let num_str = stripped.strip_suffix(".dex").unwrap_or("0");
    num_str.parse().unwrap_or(0)
}
