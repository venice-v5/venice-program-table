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

fn load(blob: &[u8]) -> Result<(), VptDefect> {
    let vpt = Vpt::new(blob, VENDOR_ID)?;
    for program in vpt.program_iter() {
        let name_bytes = program.name();
        let payload = program.payload();

        // Optionally interpret name as UTF-8 if your naming guarantees UTF-8.
        if let Ok(name) = core::str::from_utf8(name_bytes) {
            // e.g., dispatch on module name
            if name == "main" {
                // hand off payload to your MicroPython loader
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

### Constructing from a pointer (advanced)

If your blob resides in device memory and you only have a pointer, you can validate directly:

```rust
use venice_program_table::{Vpt, VptDefect};

unsafe fn load_from_ptr(ptr: *const u8, vendor_id: u32) -> Result<Vpt<'static>, VptDefect> {
    // Safety: caller must ensure `ptr` points to a valid VPT.
    Vpt::from_ptr(ptr, vendor_id)
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
    // Prepare your compiled MicroPython bytecode per module.
    let main_bc: Vec<u8> = compile_to_micropython_bytecode("print('hello')");

    let mut builder = VptBuilder::new(VENDOR_ID);
    builder.add_program(ProgramBuilder {
        name: b"main".to_vec(),
        payload: main_bc,
    });

    // Serialize to a single aligned binary blob.
    builder.build()
}

// Stub to illustrate; replace with your actual compilation pipeline.
fn compile_to_micropython_bytecode(_src: &str) -> Vec<u8> { vec![0x00, 0x01, 0x02] }
```

Notes:
- The builder handles headers, ordering, and 8-byte alignment padding for you.
- Module names are arbitrary byte strings (not NUL-terminated). Use consistent encoding (e.g., UTF-8) if you plan to display or compare as strings.
- Use the same `vendor_id` at build and load time to prevent accidental cross-loading.

## Validation Guarantees

On `Vpt::new` / `Vpt::from_ptr`, the following checks are performed:
- Magic number matches `VPT_MAGIC`.
- Version is compatible with the crate’s `VERSION`:
  - Same major.
  - If major == 0, the blob’s minor can be newer or equal to the consumer’s minor.
  - If major > 0, minor must match exactly.
- `vendor_id` matches the expected value you pass in.
- Buffer length is sufficient for the declared total size.
- Entries are iterated using explicit length fields and 8-byte alignment.

If any check fails, you receive a `VptDefect` indicating the reason.

## Tips and Patterns

- Fast lookup by name:
  - Maintain a small index (e.g., a perfect hash or sorted names array) on the host and ship it separately if you need O(log n) lookup at runtime, or just linearly scan with `Iterator::find` if your module count is small.
- Zero-copy payloads:
  - The `payload()` slice references the original blob; avoid copying unless you must mutate.
- Endianness:
  - Ensure producer and consumer agree on endianness across platforms; the common case is little-endian on both host and target.
- Safety:
  - Use `Vpt::from_ptr` only when you can guarantee alignment and that the pointer refers to a valid VPT region.

## FAQ

- How do I handle non-UTF8 names?
  - Treat names as raw bytes (`&[u8]`). Convert to `&str` only when you know they’re UTF-8.
- Can I store metadata per module?
  - The base format stores only the name and payload. If you need metadata, embed it within the payload or define a convention for a special module that carries a metadata table.
- How do I support multiple runtimes?
  - Use distinct `vendor_id` values for each consumer and build separate blobs (or a superset blob parsed differently by each consumer).
