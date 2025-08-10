# SDK Guide

This guide covers the public API for producing and consuming Venice Program Table (VPT) blobs. VPT is a compact, aligned, versioned format for shipping a mapping of module names → MicroPython bytecode to the Venice runtime on VEX V5.

- no_std by default: parsing/consuming requires no allocator.
- Optional builder feature: enable "builder" to construct VPT blobs on the host (uses alloc).

## Crate Features

- Default (no_std): read/validate/iterate VPT blobs on embedded targets.
- Feature "builder": adds a simple builder API that uses `Vec<u8>` to serialize VPT blobs.

In Cargo.toml:
```toml
[dependencies]
venice-program-table = { version = "...", features = ["builder"] }
```

If you only consume VPT blobs (embedded/firmware side), omit the feature and keep no_std.

## Constants and Core Types

- `VPT_MAGIC: u32` — magic number identifying VPT blobs.
- `VERSION: Version` — the format version the crate is built against.
- `Version` — opaque version struct; primarily used in error reporting.
- `VptDefect` — errors when validating a VPT:
  - `SizeMismatch`
  - `AlignmentMismatch`
  - `MagicMismatch(u32)`
  - `VersionMismatch(Version)`
  - `VendorMismatch(u32)`
- `Vpt<'a>` — a validated, zero-copy view of a VPT.
- `Program<'a>` — a single named payload with:
  - `name(&self) -> &[u8]`
  - `payload(&self) -> &[u8]`

## Consuming a VPT (no_std)

Validate a blob and iterate its entries without allocations:

```rust
use venice_program_table::{Vpt, VptDefect};

const VENDOR_ID: u32 = 0x1234_5678;

static BLOB: &[u8] = include_bytes!("blob.vpt");

fn load() -> Result<(), VptDefect> {
    let vpt = Vpt::new(BLOB, VENDOR_ID)?;
    for program in vpt.program_iter() {
        let name_bytes = program.name();
        let payload = program.payload();

        // Optionally interpret name as UTF-8 if your naming guarantees UTF-8.
        if let Ok(name) = core::str::from_utf8(name_bytes) {
            // e.g., dispatch on module name
            if name == "main" {
                // hand off payload to your language runtime
            }
        }
    }
    Ok(())
}
```

Lookup helper (module name as bytes):
```rust
use venice_program_table::Vpt;

fn find_program<'a>(vpt: &'a Vpt<'a>, name: &[u8]) -> Option<venice_program_table::Program<'a>> {
    vpt.program_iter().find(|p| p.name() == name)
}
```

### Constructing from a pointer/linked file (advanced)

If your blob is linked to your program at a known address, you can validate it directly:

```rust
use venice_program_table::{Vpt, VptDefect};

unsafe extern "C" {
    static __linked_file_start: u8;
}

unsafe fn load_from_linked(vendor_id: u32) -> Result<Vpt<'static>, VptDefect> {
    // Safety: `__linked_file_start` must point to a valid VPT.
    Vpt::from_ptr(&raw const __linked_file_start, vendor_id)
}
```

## Building a VPT (host-side, enable "builder")

Enable the `builder` feature to construct VPT blobs from module name/payload pairs.

Public builder API:
- `ProgramBuilder { name: Vec<u8>, payload: Vec<u8> }`
- `VptBuilder::new(vendor_id: u32) -> VptBuilder`
- `VptBuilder::add_program(&mut self, program: ProgramBuilder)`
- `VptBuilder::build(self) -> Vec<u8>`

Minimal example:
```rust
use venice_program_table::{VptBuilder, ProgramBuilder};

const VENDOR_ID: u32 = 0x1234_5678;

fn build_blob() -> Vec<u8> {
    // Prepare entrypoint name and source
    let entrypoint_name = b"main.py".to_vec();
    let entrypoint_src: Vec<u8> = b"print('hello')".to_vec();

    let mut builder = VptBuilder::new(VENDOR_ID);
    // Add entrypoint
    builder.add_program(ProgramBuilder {
        name: entrypoint_name
        payload: entrypoint_src,
    });

    // Serialize to a single aligned binary blob.
    builder.build()
}
```

Notes:
- The builder handles headers, ordering, and 8-byte alignment padding for you.
- Program names are arbitrary byte strings (not NUL-terminated).
- Use the same `vendor_id` at build and load time to prevent accidental cross-loading.

## Validation Guarantees

On `Vpt::new` / `Vpt::from_ptr`, the following checks are performed:
- Magic number matches `VPT_MAGIC`.
- Version is compatible with the SDK's `VERSION`:
  - Same major.
  - If major == 0, minor must match exactly.
  - If major > 0, the blob’s minor can be newer or equal to the consumer’s minor.
- `vendor_id` matches the expected value you pass in.
- Buffer length is sufficient for the declared total size.

If any check fails, you receive a `VptDefect` indicating the reason.

## Tips and Patterns

- Fast lookup by name:
  - Upon parsing the blob, iterate through each program and build a hash map that matches program names to their payloads for a fast O(1) average lookup.
- Zero-copy payloads:
  - The `payload()` slice references the original blob; avoid copying unless you must mutate.
- Endianness:
  - Ensure the producer uses little-endian integers.
- Safety:
  - Use `Vpt::from_ptr` only when the memory region is designated for VPT blobs and is well-aligned.

## FAQ

- Can I store metadata per program?
  - The base format stores only the name and payload. If you need metadata, embed it within the payload or define a convention for a special program that carries a metadata table.
- How do I support multiple runtimes?
  - Use distinct `vendor_id` values for each consumer and build separate blobs (or a superset blob parsed differently by each consumer).
