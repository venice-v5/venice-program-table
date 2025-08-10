#![no_std]

#[cfg(feature = "builder")]
extern crate alloc;

#[cfg(feature = "builder")]
mod builder;

use bytemuck::{AnyBitPattern, NoUninit, PodCastError, Zeroable};

#[cfg(feature = "builder")]
pub use crate::builder::{ProgramBuilder, VptBuilder};

pub const VPT_MAGIC: u32 = 0x675c3ed9;
pub const VERSION: Version = Version { major: 0, minor: 1 };

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct Version {
    major: u32,
    minor: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VptDefect {
    SizeMismatch,
    AlignmentMismatch,
    MagicMismatch(u32),
    VersionMismatch(Version),
    VendorMismatch(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, align(8))]
pub struct VptHeader {
    /// Magic number. Must be equal to [`VPT_MAGIC`], or 0x675c3ed9.
    pub magic: u32,
    /// VPT version.
    pub version: Version,
    /// ID to distinguish VPTs with different purposes. For example, it would be useful to use for
    /// a VPT containing Python code to use a different ID than one containing Lua code.
    pub vendor_id: u32,
    /// VPT payload size.
    pub size: u32,
    /// Number of programs contained within the VPT.
    pub program_count: u32,
}

unsafe impl Zeroable for VptHeader {}
unsafe impl AnyBitPattern for VptHeader {}
unsafe impl NoUninit for VptHeader {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vpt<'a> {
    // Invariant: `bytes` contains a well-aligned VPT with a valid header.
    bytes: &'a [u8],
}

// Program Layout:
//
// #[repr(C, align(8)))]
// struct Program {
//     name_len: u32,
//     payload_len: u32,
//     payload: [u8, payload_len],
//     name: [u8, name_len],
// }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, align(8))]
pub struct ProgramHeader {
    pub name_len: u32,
    pub payload_len: u32,
}

unsafe impl Zeroable for ProgramHeader {}
unsafe impl AnyBitPattern for ProgramHeader {}
unsafe impl NoUninit for ProgramHeader {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Program<'a> {
    name: &'a [u8],
    payload: &'a [u8],
}

#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramIter<'a> {
    // copy directly from VPT and don't modify
    program_count: u32,
    current_program: u32,
    bytes: &'a [u8],
}

impl Version {
    pub const fn compatible_with(&self, other: &Version) -> bool {
        self.major == other.major
            && if self.major == 0 {
                self.minor == other.minor
            } else {
                self.minor <= other.minor
            }
    }
}

impl<'a> Vpt<'a> {
    pub fn new(bytes: &'a [u8], vendor_id: u32) -> Result<Self, VptDefect> {
        if bytes.len() < size_of::<VptHeader>() {
            return Err(VptDefect::SizeMismatch);
        }

        let header: &VptHeader = bytemuck::try_from_bytes(&bytes[..size_of::<VptHeader>()])
            .map_err(|err| match err {
                PodCastError::AlignmentMismatch => VptDefect::AlignmentMismatch,
                _ => unreachable!(),
            })?;

        if header.magic != VPT_MAGIC {
            return Err(VptDefect::MagicMismatch(header.magic));
        }

        if !VERSION.compatible_with(&header.version) {
            return Err(VptDefect::VersionMismatch(header.version));
        }

        if header.vendor_id != vendor_id {
            return Err(VptDefect::VendorMismatch(header.vendor_id));
        }

        if bytes.len() < header.size as usize {
            return Err(VptDefect::SizeMismatch);
        }

        // All invariants have been checked.

        Ok(Self {
            bytes: &bytes[..header.size as usize],
        })
    }

    /// # Safety
    ///
    /// `ptr` must point to a valid VPT
    pub unsafe fn from_ptr(ptr: *const u8, vendor_id: u32) -> Result<Self, VptDefect> {
        let header_ptr = ptr as *const VptHeader;
        if !header_ptr.is_aligned() {
            return Err(VptDefect::AlignmentMismatch);
        }

        let header = unsafe { &*header_ptr };

        if header.magic != VPT_MAGIC {
            return Err(VptDefect::MagicMismatch(header.magic));
        }

        if !VERSION.compatible_with(&header.version) {
            return Err(VptDefect::VersionMismatch(header.version));
        }

        if header.vendor_id != vendor_id {
            return Err(VptDefect::VendorMismatch(header.vendor_id));
        }

        Ok(Self {
            bytes: unsafe { core::slice::from_raw_parts(ptr, header.size as usize) },
        })
    }

    pub fn header(&self) -> &VptHeader {
        bytemuck::from_bytes(&self.bytes[..size_of::<VptHeader>()])
    }

    pub fn program_iter(&self) -> ProgramIter {
        ProgramIter {
            program_count: self.header().program_count,
            current_program: 0,
            bytes: &self.bytes[size_of::<VptHeader>()..],
        }
    }
}

impl<'a> Iterator for ProgramIter<'a> {
    type Item = Program<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_program >= self.program_count {
            return None;
        }

        let header_bytes = self.bytes.get(..size_of::<ProgramHeader>())?;
        let header: &ProgramHeader = bytemuck::from_bytes(header_bytes);

        // program excluding header
        let program = &self.bytes[size_of::<ProgramHeader>()..];

        let payload = program.get(..header.payload_len as usize)?;
        let name = program.get(
            header.payload_len as usize..header.payload_len as usize + header.name_len as usize,
        )?;

        let program_len =
            size_of::<ProgramHeader>() + header.payload_len as usize + header.name_len as usize;

        self.bytes = &self.bytes[(program_len + 7) & !7..];
        self.current_program += 1;

        Some(Program { name, payload })
    }
}

impl Program<'_> {
    pub const fn name(&self) -> &[u8] {
        self.name
    }

    pub const fn payload(&self) -> &[u8] {
        self.payload
    }
}
