---
title: Introduction
---
# Introduction

The Venice Program Table (VPT) is a compact, versioned binary container for bundling multiple Python modules (by name) together with their compiled MicroPython bytecode. It is designed for the Venice MicroPython runtime running on the VEX V5, enabling you to package and deploy a “module → bytecode” mapping as a single buffer that the runtime can validate and load efficiently.

## Why VPT?

- Single artifact: Ship a single blob that contains all your precompiled modules.
- Zero-copy access: The runtime reads module names and payloads directly from the buffer without extra allocations.
- no_std friendly: Works in constrained targets; no allocator is required to parse/consume.
- Robustness: Built-in magic, version, and vendor identifiers to detect wrong or incompatible blobs early.
- Alignment-aware: Entries are padded to 8-byte alignment for predictable, safe access on the target.

## High-level Structure

A VPT consists of:
- A header that identifies the blob and describes its contents:
  - Magic number
  - Format version
  - Vendor ID (lets you differentiate tables built for different consumers/purposes)
  - Total size
  - Number of contained modules
- A sequence of “program” entries (one per module):
  - A small per-entry header with the lengths of the module name and the payload
  - The module’s bytecode payload (for the Venice MicroPython runtime)
  - The module’s name (as bytes)
  - Padding to maintain 8-byte alignment

In this documentation and API, a “program” is simply a named payload: the payload is the module’s bytecode, and the name is the module name used by the runtime.

## Producing and Consuming VPT

- Building (host side):
  - Compile your Python modules to MicroPython bytecode.
  - Use the optional builder to add each module’s name and bytecode and then serialize a VPT blob.
  - Embed the blob in firmware or distribute it alongside your application.

- Loading (target side):
  - Provide the blob and its expected vendor ID to the runtime.
  - The runtime validates the header (magic, version compatibility, vendor ID, size).
  - Iterate entries to locate modules by name and pass their bytecode payloads to Venice MicroPython.

## Design Notes

- Versioned format to support evolution while preserving compatibility.
- Strict, explicit layout and 8-byte alignment to work reliably on embedded targets.
- Zero-copy iteration over entries for minimal overhead.

Use VPT whenever you need a reliable, portable way to deliver a bundle of named MicroPython modules to the Venice runtime on VEX V5.
