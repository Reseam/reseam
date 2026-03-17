use crate::error::Result;
use crate::model::dex_file::DexFile;
use crate::model::header::ParseOptions;

/// A container for multiple DEX files, typically extracted from an APK.
#[derive(Debug)]
pub struct MultiDexContainer {
    pub dex_files: Vec<DexFile>,
}

impl MultiDexContainer {
    /// Creates an empty container.
    pub fn new() -> Self {
        Self {
            dex_files: Vec::new(),
        }
    }

    /// Parses multiple DEX buffers into one container.
    ///
    /// # Examples
    ///
    /// ```
    /// use stitch_dex::{MultiDexContainer, ParseOptions};
    ///
    /// let buffers: Vec<&[u8]> = Vec::new();
    /// let container = MultiDexContainer::parse(&buffers, ParseOptions::default())?;
    /// assert!(container.is_empty());
    /// # Ok::<(), stitch_dex::DexError>(())
    /// ```
    pub fn parse(buffers: &[&[u8]], opts: ParseOptions) -> Result<Self> {
        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            let results: std::result::Result<Vec<_>, _> = buffers
                .par_iter()
                .map(|buf| crate::reader::parse::parse(buf, opts.clone()))
                .collect();
            Ok(Self {
                dex_files: results?,
            })
        }

        #[cfg(not(feature = "parallel"))]
        {
            let mut dex_files = Vec::with_capacity(buffers.len());
            for buf in buffers {
                dex_files.push(crate::reader::parse::parse(buf, opts.clone())?);
            }
            Ok(Self { dex_files })
        }
    }

    /// Serializes every contained DEX file back into its own byte buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use stitch_dex::{MultiDexContainer, ParseOptions};
    ///
    /// let buffers: Vec<&[u8]> = Vec::new();
    /// let container = MultiDexContainer::parse(&buffers, ParseOptions::default())?;
    /// let rewritten = container.write_all()?;
    /// assert!(rewritten.is_empty());
    /// # Ok::<(), stitch_dex::DexError>(())
    /// ```
    pub fn write_all(&mut self) -> Result<Vec<Vec<u8>>> {
        let mut buffers = Vec::with_capacity(self.dex_files.len());
        for dex in &mut self.dex_files {
            buffers.push(crate::writer::write::write(dex)?);
        }
        Ok(buffers)
    }

    /// Returns the number of contained DEX files.
    pub fn len(&self) -> usize {
        self.dex_files.len()
    }

    /// Returns whether the container holds no DEX files.
    pub fn is_empty(&self) -> bool {
        self.dex_files.is_empty()
    }

    /// Returns a shared reference to a DEX file by index.
    pub fn dex(&self, index: usize) -> Option<&DexFile> {
        self.dex_files.get(index)
    }

    /// Returns a mutable reference to a DEX file by index.
    pub fn dex_mut(&mut self, index: usize) -> Option<&mut DexFile> {
        self.dex_files.get_mut(index)
    }

    /// Finds a class by descriptor across all contained DEX files.
    pub fn find_class(&self, descriptor: &str) -> Option<(usize, &crate::model::class::ClassDef)> {
        for (i, dex) in self.dex_files.iter().enumerate() {
            if let Some(class) = dex.find_class(descriptor) {
                return Some((i, class));
            }
        }
        None
    }

    /// Finds a mutable class by descriptor across all contained DEX files.
    pub fn find_class_mut(
        &mut self,
        descriptor: &str,
    ) -> Option<(usize, &mut crate::model::class::ClassDef)> {
        for (i, dex) in self.dex_files.iter_mut().enumerate() {
            if let Some(class) = dex.find_class_mut(descriptor) {
                return Some((i, class));
            }
        }
        None
    }

    /// Appends a DEX file to the container.
    pub fn add_dex(&mut self, dex: DexFile) {
        self.dex_files.push(dex);
    }

    /// Removes and returns a DEX file by index.
    pub fn remove_dex(&mut self, index: usize) -> DexFile {
        self.dex_files.remove(index)
    }

    /// Returns an iterator over contained DEX files.
    pub fn iter(&self) -> std::slice::Iter<'_, DexFile> {
        self.dex_files.iter()
    }

    /// Returns a mutable iterator over contained DEX files.
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
