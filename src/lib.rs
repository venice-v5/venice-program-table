//! # Venice Program Table SDK
//!
//! The Venice Program Table (VPT) is a data structure designed for delivering program modules to
//! language runtimes running on VEX V5 robots.
//!
//! This crate, the VPT SDK, is the standard implementation of a VPT parser and builder. This SDK
//! is `no_std`, meaning it can run on targets without `std` support, such as `armv7a-vex-v5`, the
//! Rust target for V5 programs.
//!
//! The SDK also optionally includes a builder, gated by the `builder` feature, which requires a
//! memory allocator.
//!
//! Additionally, the SDK, excluding the builder, is entirely zero-copy. [`Vpt`]s and [`Program`]s
//! reference the original blob's memory, and inherit its lifetime. So, if a blob is stored as
//! static data or is dynamically linked to the program, its data will be available for the
//! entirety of the program without.

#![no_std]
#![warn(missing_docs)]

#[cfg(feature = "builder")]
extern crate alloc;

#[cfg(feature = "builder")]
mod builder;

use bytemuck::{AnyBitPattern, NoUninit, PodCastError, Zeroable};
use thiserror::Error;

#[cfg(feature = "builder")]
pub use crate::builder::{ProgramBuilder, VptBuilder};

/// Magic number used to identify VPTs.
pub const VPT_MAGIC: u32 = 0x675c3ed9;

/// VPT version this SDK is built against.
pub const SDK_VERSION: Version = Version { major: 0, minor: 1 };

const fn align8(n: usize) -> usize {
    (n + 7) & !7
}

/// A version of the VPT spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct Version {
    /// Major version number.
    pub major: u32,
    /// Minor version number.
    pub minor: u32,
}

/// An error encountered while validating a VPT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum VptDefect {
    /// The blob is longer than the provided bytes.
    #[error("VPT blob longer than provided bytes")]
    SizeMismatch,
    /// The blob is not 8-byte aligned.
    #[error("VPT blob not 8-byte aligned")]
    AlignmentMismatch,
    /// `header.magic` does not match [`VPT_MAGIC`], or 0x675c3ed9.
    #[error("incorrect magic: expected 0x675c3ed9, found 0x{0:08x}")]
    MagicMismatch(u32),
    /// `header.version` is incompatible with [`SDK_VERSION`].
    #[error("incompatible version")]
    VersionMismatch(Version),
    /// `header.vendor_id` does not match the provided vendor ID.
    #[error("vendor ID mismatch")]
    VendorMismatch(u32),
}

/// VPT Header
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, align(8))]
pub struct VptHeader {
    /// Magic number. Must be equal to [`VPT_MAGIC`], or 0x675c3ed9.
    pub magic: u32,
    /// VPT version.
    pub version: Version,
    /// Vendor ID.
    pub vendor_id: u32,
    /// VPT payload size.
    pub size: u32,
    /// Number of programs contained within the VPT.
    pub program_count: u32,
}

unsafe impl Zeroable for VptHeader {}
unsafe impl AnyBitPattern for VptHeader {}
unsafe impl NoUninit for VptHeader {}

/// A read-only view of a validated VPT.
///
/// This VPT has been verified to be version-compatible with SDK, well-aligned, and contain a
/// matching vendor ID. Its programs can be iterated through by calling [`program_iter`], returning
/// a [`ProgramIter`] which iterates through each contained program, until it either has iterated
/// through `header.program_count` programs, or it has exhausted the total number of bytes in the
/// blob.
///
/// [`program_iter`]: `Vpt::program_iter`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vpt<'a> {
    // Invariant: `bytes` contains a well-aligned VPT with a valid header.
    bytes: &'a [u8],
}

/// Program Header
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, align(8))]
pub struct ProgramHeader {
    /// Length of the program's name in bytes.
    pub name_len: u32,
    /// Length of the program's payload in bytes.
    pub payload_len: u32,
}

unsafe impl Zeroable for ProgramHeader {}
unsafe impl AnyBitPattern for ProgramHeader {}
unsafe impl NoUninit for ProgramHeader {}

/// A read-only view of a program's name and payload. This view has the same lifetime as the [`Vpt`]
/// it originated from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Program<'a> {
    name: &'a [u8],
    payload: &'a [u8],
}

/// VPT program iterator obtained from [`Vpt::program_iter`]. This iterator will continue to
/// iterate through its VPT until either `header.program_count` has been exceeded or the blob's
/// bytes have been exhausted.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramIter<'a> {
    // copy directly from VPT and don't modify
    program_count: u32,
    current_program: u32,
    bytes: &'a [u8],
}

impl Version {
    /// Checks the compatibility of two versions, according to the VPT spec.
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
    /// Constructs a [`Vpt`] from a byte slice.
    ///
    /// # Errors
    ///
    /// - [`VptDefect::SizeMismatch`] if `bytes` could not contain the entire VPT.
    /// - [`VptDefect::AlignmentMismatch`] if `bytes` is not 8-byte aligned.
    /// - [`VptDefect::MagicMismatch`] if `header.magic` does not match [`VPT_MAGIC`].
    /// - [`VptDefect::VersionMismatch`] if `header.version` is not compatible with [`SDK_VERSION`].
    /// - [`VptDefect::VendorMismatch`] if `header.vendor_id` does not match `vendor_id`.
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

        if !SDK_VERSION.compatible_with(&header.version) {
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

    /// Constructs a [`Vpt`] from a pointer.
    ///
    /// # Errors
    ///
    /// - [`VptDefect::AlignmentMismatch`] if `ptr` is not 8-byte aligned.
    /// - [`VptDefect::MagicMismatch`] if `header.magic` does not match [`VPT_MAGIC`].
    /// - [`VptDefect::VersionMismatch`] if `header.version` is not compatible with [`SDK_VERSION`].
    /// - [`VptDefect::VendorMismatch`] if `header.vendor_id` does not match `vendor_id`.
    ///
    /// # Safety
    ///
    /// `ptr` must point to memory that is valid for reading up to `header.size` bytes.
    pub unsafe fn from_ptr(ptr: *const u8, vendor_id: u32) -> Result<Self, VptDefect> {
        let header_ptr = ptr as *const VptHeader;
        if !header_ptr.is_aligned() {
            return Err(VptDefect::AlignmentMismatch);
        }

        let header = unsafe { &*header_ptr };

        if header.magic != VPT_MAGIC {
            return Err(VptDefect::MagicMismatch(header.magic));
        }

        if !SDK_VERSION.compatible_with(&header.version) {
            return Err(VptDefect::VersionMismatch(header.version));
        }

        if header.vendor_id != vendor_id {
            return Err(VptDefect::VendorMismatch(header.vendor_id));
        }

        Ok(Self {
            bytes: unsafe { core::slice::from_raw_parts(ptr, header.size as usize) },
        })
    }

    /// Returns the [`VptHeader`] of the VPT.
    pub fn header(&self) -> &VptHeader {
        bytemuck::from_bytes(&self.bytes[..size_of::<VptHeader>()])
    }

    /// Returns a [`ProgramIter`] which can be used to iterate through the programs within the VPT.
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

        self.bytes = &self.bytes[align8(program_len)..];
        self.current_program += 1;

        Some(Program { name, payload })
    }
}

impl<'a> Program<'a> {
    /// Returns the name of the program.
    pub const fn name(&self) -> &'a [u8] {
        self.name
    }

    /// Returns the payload of the program.
    pub const fn payload(&self) -> &'a [u8] {
        self.payload
    }
}
