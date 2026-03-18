use crate::error::Result;
use crate::file::DexFile;
use crate::types::header::ParseOptions;

#[derive(Debug)]
pub struct MultiDexContainer {
    pub dex_files: Vec<DexFile>,
}

impl MultiDexContainer {
    pub fn new() -> Self {
        Self {
            dex_files: Vec::new(),
        }
    }

    pub fn parse(buffers: &[&[u8]], opts: ParseOptions) -> Result<Self> {
        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            let results: std::result::Result<Vec<_>, _> = buffers
                .par_iter()
                .map(|buf| crate::read::parse::parse(buf, opts.clone()))
                .collect();
            Ok(Self {
                dex_files: results?,
            })
        }

        #[cfg(not(feature = "parallel"))]
        {
            let mut dex_files = Vec::with_capacity(buffers.len());
            for buf in buffers {
                dex_files.push(crate::read::parse::parse(buf, opts.clone())?);
            }
            Ok(Self { dex_files })
        }
    }

    pub fn parse_container(buf: &[u8], opts: ParseOptions) -> Result<Self> {
        let dex_files = crate::read::parse::parse_container(buf, opts)?;
        Ok(Self { dex_files })
    }

    pub fn write_all(&mut self) -> Result<Vec<Vec<u8>>> {
        let mut buffers = Vec::with_capacity(self.dex_files.len());
        for dex in &mut self.dex_files {
            buffers.push(crate::write::write(dex)?);
        }
        Ok(buffers)
    }

    pub fn write_container(&mut self) -> Result<Vec<u8>> {
        crate::write::write_container(&mut self.dex_files)
    }

    pub fn len(&self) -> usize {
        self.dex_files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dex_files.is_empty()
    }

    pub fn dex(&self, index: usize) -> Option<&DexFile> {
        self.dex_files.get(index)
    }

    pub fn dex_mut(&mut self, index: usize) -> Option<&mut DexFile> {
        self.dex_files.get_mut(index)
    }

    pub fn find_class(&self, descriptor: &str) -> Option<(usize, &crate::types::class::ClassDef)> {
        for (i, dex) in self.dex_files.iter().enumerate() {
            if let Some(class) = dex.find_class(descriptor) {
                return Some((i, class));
            }
        }
        None
    }

    pub fn find_class_mut(
        &mut self,
        descriptor: &str,
    ) -> Option<(usize, &mut crate::types::class::ClassDef)> {
        for (i, dex) in self.dex_files.iter_mut().enumerate() {
            if let Some(class) = dex.find_class_mut(descriptor) {
                return Some((i, class));
            }
        }
        None
    }

    pub fn add_dex(&mut self, dex: DexFile) {
        self.dex_files.push(dex);
    }

    pub fn remove_dex(&mut self, index: usize) -> DexFile {
        self.dex_files.remove(index)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, DexFile> {
        self.dex_files.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, DexFile> {
        self.dex_files.iter_mut()
    }
}

impl Default for MultiDexContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> IntoIterator for &'a MultiDexContainer {
    type Item = &'a DexFile;
    type IntoIter = std::slice::Iter<'a, DexFile>;

    fn into_iter(self) -> Self::IntoIter {
        self.dex_files.iter()
    }
}

impl<'a> IntoIterator for &'a mut MultiDexContainer {
    type Item = &'a mut DexFile;
    type IntoIter = std::slice::IterMut<'a, DexFile>;

    fn into_iter(self) -> Self::IntoIter {
        self.dex_files.iter_mut()
    }
}
