use crate::error::{Error, Kind, Result};
use crate::util::{u16_at, u32_at};

pub const RT_CURSOR: u32 = 1;

pub const RT_GROUP_CURSOR: u32 = 12;

const KIND: Kind = Kind::Executable;

#[derive(Debug, Clone, Copy)]

struct Section {
    rva: u32,
    virtual_size: u32,
    raw_offset: u32,
    raw_size: u32,
}

#[derive(Debug)]

pub struct Resources<'a> {
    image: &'a [u8],
    sections: Vec<Section>,
    root: usize,
}

impl<'a> Resources<'a> {
    pub fn parse(image: &'a [u8]) -> Result<Resources<'a>> {
        if image.get(..2) != Some(b"MZ") {
            return Err(Error::structure(KIND, "not an MZ image"));
        }

        let e_lfanew = u32_at(image, 0x3C, KIND)? as usize;

        if image.get(e_lfanew..e_lfanew + 4) != Some(b"PE\0\0") {
            return Err(Error::structure(
                KIND,
                format!("no PE header at {e_lfanew:#x}"),
            ));
        }

        let coff = e_lfanew + 4;

        let section_count = u16_at(image, coff + 2, KIND)? as usize;

        let optional_size = u16_at(image, coff + 16, KIND)? as usize;

        let optional = coff + 20;

        let magic = u16_at(image, optional, KIND)?;

        if magic != 0x010B {
            return Err(Error::structure(
                KIND,
                format!("optional header magic {magic:#06x} is not PE32"),
            ));
        }

        let directory_count = u32_at(image, optional + 92, KIND)?;

        if directory_count < 3 {
            return Err(Error::structure(
                KIND,
                "image has no resource data directory",
            ));
        }

        let resource_rva = u32_at(image, optional + 96 + 2 * 8, KIND)?;

        if resource_rva == 0 {
            return Err(Error::structure(
                KIND,
                "image has an empty resource directory",
            ));
        }

        let mut sections = Vec::with_capacity(section_count);

        let table = optional + optional_size;

        for index in 0..section_count {
            let entry = table + index * 40;

            sections.push(Section {
                virtual_size: u32_at(image, entry + 8, KIND)?,
                rva: u32_at(image, entry + 12, KIND)?,
                raw_size: u32_at(image, entry + 16, KIND)?,
                raw_offset: u32_at(image, entry + 20, KIND)?,
            });
        }

        let mut resources = Resources {
            image,
            sections,
            root: 0,
        };

        resources.root = resources.offset(resource_rva).ok_or_else(|| {
            Error::structure(
                KIND,
                format!("resource RVA {resource_rva:#x} is in no section"),
            )
        })?;

        Ok(resources)
    }

    pub fn offset(&self, rva: u32) -> Option<usize> {
        for section in &self.sections {
            let extent = section.virtual_size.max(section.raw_size);

            if rva < section.rva || rva - section.rva >= extent {
                continue;
            }

            let within = rva - section.rva;

            if within >= section.raw_size {
                return None;
            }

            return Some(section.raw_offset as usize + within as usize);
        }

        None
    }

    pub fn find(&self, kind: u32, id: u32) -> Result<Option<&'a [u8]>> {
        let Some(by_id) = self.subdirectory(self.root, kind)? else {
            return Ok(None);
        };

        let Some(by_language) = self.subdirectory(by_id, id)? else {
            return Ok(None);
        };

        let Some(leaf) = self.first_entry(by_language)? else {
            return Ok(None);
        };

        let data_rva = u32_at(self.image, leaf, KIND)?;

        let size = u32_at(self.image, leaf + 4, KIND)? as usize;

        let start = self.offset(data_rva).ok_or_else(|| {
            Error::structure(
                KIND,
                format!("resource data RVA {data_rva:#x} is in no section"),
            )
        })?;

        let end = start.checked_add(size).ok_or(Error::Truncated {
            kind: KIND,
            offset: start,
            needed: size,
            len: self.image.len(),
        })?;

        self.image
            .get(start..end)
            .map(Some)
            .ok_or(Error::Truncated {
                kind: KIND,
                offset: start,
                needed: size,
                len: self.image.len(),
            })
    }

    pub fn ids(&self, kind: u32) -> Result<Vec<u32>> {
        let Some(by_id) = self.subdirectory(self.root, kind)? else {
            return Ok(Vec::new());
        };

        let (named, numbered) = self.counts(by_id)?;

        let mut ids = Vec::with_capacity(numbered);

        for index in named..named + numbered {
            let entry = by_id + 16 + index * 8;

            ids.push(u32_at(self.image, entry, KIND)? & 0x7FFF_FFFF);
        }

        Ok(ids)
    }

    fn counts(&self, at: usize) -> Result<(usize, usize)> {
        let named = u16_at(self.image, at + 12, KIND)? as usize;

        let numbered = u16_at(self.image, at + 14, KIND)? as usize;

        Ok((named, numbered))
    }

    fn subdirectory(&self, at: usize, id: u32) -> Result<Option<usize>> {
        let (named, numbered) = self.counts(at)?;

        for index in named..named + numbered {
            let entry = at + 16 + index * 8;

            if u32_at(self.image, entry, KIND)? & 0x7FFF_FFFF != id {
                continue;
            }

            let offset = u32_at(self.image, entry + 4, KIND)?;

            if offset & 0x8000_0000 == 0 {
                return Err(Error::structure(
                    KIND,
                    format!("resource entry {id} is a leaf where a directory was expected"),
                ));
            }

            return Ok(Some(self.root + (offset & 0x7FFF_FFFF) as usize));
        }

        Ok(None)
    }

    fn first_entry(&self, at: usize) -> Result<Option<usize>> {
        let (named, numbered) = self.counts(at)?;

        if named + numbered == 0 {
            return Ok(None);
        }

        let offset = u32_at(self.image, at + 16 + 4, KIND)?;

        if offset & 0x8000_0000 != 0 {
            return Err(Error::structure(
                KIND,
                "resource language entry is a directory where a leaf was expected",
            ));
        }

        Ok(Some(self.root + offset as usize))
    }
}
