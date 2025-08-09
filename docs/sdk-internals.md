# Internals

This document explains how the Venice Program Table (VPT) crate is implemented, the invariants it relies on, and what to watch out for when modifying or extending it.

The crate is `no_std` by default and exposes a zero-copy reader. A feature-gated builder (`feature = "builder"`) uses `alloc` to construct blobs.

## Core layout and invariants

- File format
  - VPT is a contiguous, 8-byte–aligned binary blob:
    - A single `VptHeader` (repr(C), align(8))
    - Followed by `program_count` program entries, each aligned to 8 bytes
  - Per-entry layout:
    - `ProgramHeader { name_len: u32, payload_len: u32 }` (repr(C), align(8)) — size 8
    - `payload` bytes
    - `name` bytes
    - zero padding so that the next entry begins at an 8-byte boundary
- Header sizes
  - `VptHeader` has 6 × u32 = 24 bytes; with align(8), 24 is still a multiple of 8
  - `ProgramHeader` is exactly 8 bytes
- Alignment
  - The header is read at an 8-byte–aligned address
  - Each program entry start is at an 8-byte boundary
  - Padding is always composed of zero bytes, but consumers do not depend on inspecting these bytes (only on alignment math)
- Size accounting
  - `header.size` equals `sizeof(VptHeader) + Σ align8(sizeof(ProgramHeader) + payload_len + name_len)`
  - `align8(n) = (n + 7) & !7`

## Safety and validation

- Reader entry point: `Vpt::new(bytes, vendor_id) -> Result<Vpt, VptDefect>`
  - Pre-check that `bytes.len() >= sizeof(VptHeader)`
  - Interpret the header via `bytemuck::try_from_bytes` (this guarantees proper alignment and header shape)
  - Validate:
    - Magic equals `VPT_MAGIC`
    - Version is compatible with `VERSION` (same major; if major == 0, the blob’s minor can be newer-or-equal; else minor must match exactly)
    - `vendor_id` matches
    - `bytes.len() >= header.size`
  - If successful, `Vpt.bytes` is truncated to exactly `header.size`, so iteration will never overrun the declared region
- Pointer-based construction: `unsafe Vpt::from_ptr(ptr, vendor_id)`
  - Checks pointer alignment for `VptHeader`
  - Performs the same magic/version/vendor checks
  - Constructs slice with `core::slice::from_raw_parts(ptr, header.size as usize)`
  - Safety is on the caller to ensure `ptr` points to a valid, readable region of at least `header.size` bytes
- Bytemuck usage
  - `VptHeader` and `ProgramHeader` implement `Zeroable`, `AnyBitPattern`, and `NoUninit`, allowing reinterpretation from bytes when alignment is correct
  - The only plausible failure for `try_from_bytes` in our header path is alignment; length is pre-validated
- Endianness
  - Integers are serialized in the producer’s native endianness; consumers are assumed to match the same endianness (common case: little-endian)
  - If heterogeneity is required, introduce a pipeline step to re-encode or standardize on a fixed endianness in a future revision

## Iteration semantics

- `Vpt::program_iter()` yields a `ProgramIter` over the region after the header
- `ProgramIter::next()`:
  - Reads an aligned `ProgramHeader` from the current cursor
  - Slices `payload` and `name` with bound checks against the remaining bytes
  - Advances the cursor by `align8(8 + payload_len + name_len)`
  - Stops when there aren’t enough bytes left for a full `ProgramHeader`
- Iteration is zero-copy:
  - `Program { name: &[u8], payload: &[u8] }` borrow directly from the VPT slice
  - Lifetimes are tied to the `Vpt` instance
- Note: `program_count` is not enforced by the iterator; it’s a descriptive field useful for preallocation and validation by callers. Iteration will end when bytes are exhausted.

## Builder implementation notes (feature = "builder")

- `VptBuilder::new(vendor_id)` starts an empty table
- `add_program(ProgramBuilder { name, payload })` appends a program in the given order
- `build()`:
  - Computes `total_size = sizeof(VptHeader) + Σ align8(sizeof(ProgramHeader) + name.len + payload.len)`
  - Reserves exactly `total_size` in a `Vec<u8>` to avoid reallocation
  - Serializes:
    - `VptHeader` with `size = total_size` and `program_count = programs.len()`
    - For each program:
      - `ProgramHeader { name_len, payload_len }`
      - `payload` bytes
      - `name` bytes
      - zero padding to `align8`
- Padding
  - Padding is zero-filled; readers must rely on computed offsets rather than scanning for zeros
- MSRV note
  - Prefer using `Vec::resize(bytes.len() + padding, 0)` for padding to avoid depending on newer iterator helpers across toolchains

## Error model

- `VptDefect` variants:
  - `SizeMismatch` — not enough bytes for header or declared size exceeds provided buffer
  - `AlignmentMismatch` — header (or pointer in `from_ptr`) is not properly aligned
  - `MagicMismatch(u32)` — header magic does not match
  - `VersionMismatch(Version)` — version not compatible per the policy
  - `VendorMismatch(u32)` — vendor ID does not match expected
- When adding new checks, prefer precise error variants over collapsing into a generic mismatch, to aid debugging on constrained targets

## Invariants to preserve

- Struct layout and traits
  - `VptHeader` and `ProgramHeader` must remain `#[repr(C, align(8))]`
  - Keep `bytemuck` traits in sync if fields change
  - Do not reorder fields without updating docs and accounting for on-wire compatibility
- Header size
  - `size_of::<VptHeader>()` must remain a multiple of 8
  - If adding fields, ensure the header stays 8-byte aligned and the total size remains aligned
- Entry shape
  - The order within an entry is: `ProgramHeader`, `payload`, `name`, `padding`
  - `ProgramHeader` size must remain 8 bytes unless a format bump is planned
- Zero-copy pledge
  - Readers must never copy payload or name slices internally
  - Public APIs should expose borrowed slices and avoid hidden allocations
- Vendor/version checks
  - Validation must occur before exposing a `Vpt` view to callers

## Testing and verification

- Unit tests
  - Construct small VPTs with multiple entries (including empty name/payload cases) and verify iteration results and offsets
  - Verify alignment math by checking cursor positions before/after `next()`
- Fuzzing (recommended)
  - Target parser with arbitrary inputs to exercise bound checks and alignment assumptions
- Cross-check
  - Round-trip test: build with the builder, then parse and re-check all fields, names, and payloads
- Compatibility tests
  - If versioning rules evolve, include tests that simulate older/newer minor versions with major=0 and with major>0

## Performance notes

- One-pass, zero-copy parse with O(1) per entry
- Builder pre-reserves exact capacity to avoid reallocation
- No hashing or indexing in-core; callers add their own lookup structures if needed

## Evolution guidelines

- Backward compatibility
  - Any change to on-wire layout requires a version bump and a clearly documented compatibility strategy
- Feature gating
  - Keep host-side build logic behind `feature = "builder"` to preserve `no_std` consumer footprint
- Endianness
  - If future use-cases demand cross-endian interchange, formalize endianness at the format level (e.g., little-endian) and introduce encode/decode helpers
- Program count
  - Consider exposing an iterator that stops after `program_count` entries if callers want strict adherence to the header; keep the current iterator for lenient traversal

## Common pitfalls

- Forgetting alignment checks
  - Do not read typed headers from unaligned addresses; rely on `try_from_bytes` or ensure alignment is guaranteed at the iterator cursor
- Assuming UTF-8 names
  - Names are raw bytes; convert only when the producer guarantees UTF-8
- Relying on padding contents
  - Padding is zeroed by the builder today, but consumers must not depend on reading zeros; only use the computed `align8` step
- Mutating payloads or names
  - The slices point into the original blob; mutating them requires the blob to be mutable and exclusive, which the API does not provide by design

## Maintenance checklist

- When changing headers:
  - Verify `size_of` and `align_of` are as expected (multiple of 8)
  - Update docs (Advanced, SDK, and here) and increment version if needed
- When changing iteration logic:
  - Keep bound checks for both payload and name, and update the cursor with `align8`
  - Add tests for boundary conditions and truncated tables
- When changing builder:
  - Keep header fields consistent with reader expectations
  - Ensure padding count matches the reader’s `align8` math
  - Maintain single-allocation behavior where possible

By maintaining these invariants and practices, the crate remains safe for `no_std`, efficient on embedded targets, and predictable to integrate and evolve.