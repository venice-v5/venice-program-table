#![no_std]

use bytemuck::{AnyBitPattern, NoUninit, PodCastError};

pub const VPT_MAGIC: u32 = 0x675c3ed9;
pub const VERSION: Version = Version { major: 0, minor: 0 };

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoUninit, AnyBitPattern)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoUninit, AnyBitPattern)]
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

pub struct Vpt<'a> {
    // Invariant: `header` points to a valid VPT
    header: &'a VptHeader,
}

impl Version {
    pub const fn compatible_with(&self, other: &Version) -> bool {
        self.major != other.major || other.minor < self.minor
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

        Ok(Self { header })
    }

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

        Ok(Self { header })
    }

    pub const fn bytes(&self) -> &[u8] {
        // SAFETY: VPT invariant is upheld
        unsafe {
            core::slice::from_raw_parts(
                self.header as *const _ as *const _,
                self.header.size as usize,
            )
        }
    }
}
