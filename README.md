# FerriFirm

**A bare-metal x86_64 firmware implementation written in Rust.**

FerriFirm is an experimental firmware project exploring a Rust implementation of a traditional staged firmware initialization architecture.

It targets x86_64 systems and is developed and tested using QEMU.

The implementation uses `no_std`, `no_main` and direct hardware interaction through Rust and inline assembly.

There is no C implementation in the firmware source.

## Design objectives

- Bare-metal x86_64 execution.
- Rust-only firmware implementation.
- Staged hardware initialization.
- Memory discovery and resource description.
- Firmware handoff structures.
- Early memory allocation.
- Custom ROM image generation.
- QEMU-based development and verification.

## Firmware architecture

FerriFirm follows three principal initialization phases.

### SEC — Security / Early Initialization

SEC is the firmware entry phase.

Responsibilities include:

- Initial execution after reset.
- Early diagnostic output.
- Establishing the transition into PEI.

The current entry point is `sec_main()`.

### PEI — Pre-EFI Initialization

PEI establishes the early platform environment.

The implementation contains structures and routines for:

- E820 memory-map handling.
- Handoff Information Tables.
- Firmware resource descriptions.
- Memory allocation HOBs.
- Firmware configuration access.
- Handoff information for the next phase.

PEI transfers the resulting platform information to DXE.

### DXE — Driver Execution Environment

DXE consumes the PEI handoff information.

The implementation includes:

- HOB processing.
- Resource descriptor handling.
- Memory allocation structures.
- A firmware allocator primitive.
- Diagnostic and error handling.

The DXE implementation is the foundation for subsequent driver and hardware initialization work.

## Implementation

The firmware is organized into Rust modules:

```text
ferrifirm/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── .cargo/
│   └── config.toml
├── src/
│   ├── main.rs
│   ├── sec.rs
│   ├── pei.rs
│   └── dxe.rs
├── link.ld
├── build_rom.sh
└── verify.py
```

The firmware uses:

- Rust `core`.
- `#![no_std]`.
- `#![no_main]`.
- Inline x86 assembly.
- A custom linker script.
- A custom ROM build process.

## Current development status

| Component | Status |
|---|---|
| Rust bare-metal entry | Implemented |
| SEC entry and diagnostics | Implemented |
| PEI initialization | Implemented |
| Memory-map structures | Implemented |
| HOB structures | Implemented |
| DXE entry | Implemented |
| DXE allocator primitive | Tested |
| QEMU phase progression | Tested |
| Complete platform driver environment | In development |
| Production hardware support | Not yet established |

## Building

The project uses its pinned Rust toolchain configuration.

Build the firmware ROM:

```bash
cd "$HOME/Desktop/ferrifirm comp"
bash build_rom.sh
```

The build process produces firmware artifacts including:

- `ferrifirm.bin`
- `ferrifirm.rom`
- `ferrifirm.map`

The verification utility is:

```bash
python3 verify.py
```

## Testing

QEMU is the primary development environment.

A typical diagnostic run uses:

```bash
qemu-system-x86_64 \
    -machine pc \
    -m 128M \
    -bios ferrifirm.bin \
    -debugcon stdio \
    -global isa-debugcon.iobase=0xe9 \
    -no-reboot
```

The firmware has been exercised through SEC, PEI and DXE progression in QEMU.

QEMU validation is not equivalent to production hardware certification.

## Development direction

Future work includes:

- More complete platform discovery.
- Firmware resource management.
- Expanded memory services.
- Hardware initialization.
- Driver infrastructure.
- More complete firmware handoff services.
- Broader hardware compatibility.

## Project philosophy

FerriFirm explores how much of a traditional x86_64 firmware stack can be implemented directly in Rust without relying on a C firmware codebase.

Correct initialization, explicit memory ownership and well-defined firmware interfaces are central design principles.
