# Vendor Support Policy for rlvgl

This document describes how vendor-specific support is managed in the `rlvgl-platform` crates.  
It clarifies the distinction between community contributions and officially supported platforms.

---

## Core Principles

- The **core `rlvgl` library** is vendor-neutral and open source.  
- The **`rlvgl-platform` crates** provide vendor- and board-specific integration layers.  
- Platform support is tiered to reflect different levels of maintenance and guarantees.

---

## Support Tiers

### Official Support
- Maintained directly in the `rlvgl` repository.  
- Included in continuous integration (CI) builds and tests.  
- Documented in the official examples and gallery.  
- Guaranteed compatibility with each `rlvgl` release.  
- Requires vendor sponsorship or equivalent partnership agreement.

### Community Support
- May be developed and maintained by community contributors.  
- Accepted into the repository if it passes basic review and compiles.  
- Built in CI for compile-time checks only.  
- Not guaranteed to be included in documentation or examples.  
- No compatibility guarantee across `rlvgl` versions.

### External Support
- Developed and maintained outside of the `rlvgl` repository.  
- May be linked from documentation as an external resource.  
- No guarantees or responsibilities from the `rlvgl` maintainers.

---

## Vendor Participation

Vendors interested in **Official Support** should provide:
1. Sponsorship or partnership to cover ongoing maintenance.  
2. Reference hardware (evaluation kits, boards, or modules).  
3. Documentation and test material as needed.  

This ensures that vendor hardware is represented with the same stability, documentation, and polish as the simulator and other officially supported platforms.

---

## Summary

- **Anyone** can build on `rlvgl` and contribute platform code.  
- **Official status** is reserved for vendor-sponsored platforms, with full CI coverage, examples, and documentation.  
- This policy keeps the core open while ensuring sustainable support for vendor ecosystems.
