---
# https://vitepress.dev/reference/default-theme-home-page
layout: home

hero:
  name: "Venice Program Table"
  text: "Multi-purpose file format for delivering code to VEX V5 programs "
  actions:
    - theme: brand
      text: Introduction
      link: /introduction

features:
  - title: Versioned, vendor-scoped container
    details: Bundle named program modules into a single binary with magic, versioning, and vendor ID for safe loading on VEX V5.
  - title: Zero-copy, no_std parsing
    details: Iterate modules without allocations; 8-byte alignment and compact headers for embedded targets such VEX programs.
  - title: Host-side builder
    details: Build VPT blobs by adding module names and programs; serialize once and deploy anywhere.
---
