# Language Primitives and Data Layout Design

Date: 2026-05-22

## Purpose

Wrela is an AArch64-only systems language for building complete appliance
images. The language should make machine authority, memory ownership, data
layout, and hot data paths visible to the compiler without turning all code
into assembly or compiler folklore.

This design captures the initial shape of the language nucleus:

- Scalar control and authority code remains explicit and readable.
- Bulk logical data is columnar by default.
- Tables and masks provide the primary vector-friendly programming model.
- Fixed vector types and AArch64 intrinsics remain available for sharp kernels.
- Physical memory layout is declared with the `layout` keyword.

## Relationship To Test Suites

The explicit test-suite model depends on these language primitives:

- `class` for capability-carrying test suites and host/QEMU adapters.
- `interface` for behavior-shaped capabilities such as `Console`, `Clock`,
  `Memory`, and `BlockDevice`.
- `test` as a compiler-known declaration inside suite classes.
- `with` fixtures for test-local fake construction.
- Explicit root images that import and construct suite classes.

This document focuses on the broader language shape underneath that test model.

## AArch64-Only Backend

Wrela targets AArch64 only. That lets the language and compiler assume:

- AArch64 calling convention and register families.
- AArch64 memory ordering rules.
- AArch64 exception levels and boot realities.
- Advanced SIMD/NEON as the first vector target when available.
- Optional future SVE support behind explicit target requirements.

The language should still avoid baking a single microarchitecture into source
semantics. CPU features belong in image/root requirements, not in ambient
compiler assumptions.

Example:

```wrela
image PacketAppliance
    requires cpu.adv_simd
{
    phase boot(platform: unique QemuVirt) {
        let console = UartConsole(platform.uart0.claim())
        console.write("packet appliance booted")
    }
}
```

## Control Plane And Data Plane

Not all of Wrela should be vector-shaped.

Scalar, authority-heavy code includes:

- Boot.
- Capability narrowing.
- Hardware discovery.
- Page table construction.
- Interrupt routing.
- Executor placement.
- Device state machines.
- Error paths.

These paths should optimize for clarity, explicit ownership, and reviewability.

Vector-friendly code is mostly data-plane work:

- Packet classification.
- Descriptor processing.
- Checksums.
- Copy, fill, compare, and scan kernels.
- Compression or codec kernels.
- Page and block metadata scans.
- Timer or ready-set filtering.
- Queue and ring-buffer batch movement.

The language should make vectorizable regions easy to express without making
the entire program graph pretend to be SIMD.

## Logical Data

`data` declares a logical record schema. It does not promise a physical
array-of-structs layout.

```wrela
data Packet {
    src: U32
    dst: U32
    len: U16
    flags: U8
}
```

A single `Packet` value can still behave like an ordinary aggregate. But bulk
storage of `Packet` values is compiler-owned unless a physical layout is
declared.

## Tables

`Table[T]` is the primary bulk data container for logical records. A table stores
records in structure-of-arrays form by default.

For:

```wrela
data Packet {
    src: U32
    dst: U32
    len: U16
    flags: U8
}

let packets: Table[Packet, 256]
```

the logical rows are `Packet` records, but storage is columnar:

```text
src:   [U32, U32, U32, U32]
dst:   [U32, U32, U32, U32]
len:   [U16, U16, U16, U16]
flags: [U8,  U8,  U8,  U8]
```

This gives the compiler direct column loads and stores instead of hoping it can
recover vectorizable structure from array-of-structs memory.

Table fields produce column views:

```wrela
let lengths = packets.len
let sources = packets.src
```

Tables are a logical data model, not a stable object-address model. Taking the
address of a row should be restricted because there may not be an addressable
array-of-structs row in memory.

## Masks

`Mask` represents a selected set of table rows or vector lanes. Masks are the
primary way to express data-parallel control flow.

```wrela
let valid = packets.flags.has(PacketFlag.Valid)
let large = packets.len > 1200
let selected = valid & large

packets.flags[selected].set(PacketFlag.Jumbo)
```

Mask operations should support:

- Boolean combination.
- Masked loads and stores.
- Selection and blending.
- Counting selected rows.
- Filtering or compaction when physical movement is required.

This lets branches over many records become predicated operations instead of
scalar control-flow diamonds.

## Tables Plus Masks As The Main Vector Model

Tables and masks are the high-level vectorization surface. They let users write
bulk data operations in terms of records and conditions while giving the
compiler a columnar, predicated lowering target.

Example:

```wrela
fn mark_ready(timers: Table[TimerEntry], now: Tick) {
    let expired = timers.deadline <= now

    timers.state[expired] = TimerState.Ready
}
```

The compiler can lower this to scalar code, NEON-width batches, or a future SVE
strategy without changing the source-level semantics.

## Fixed Vector Types

Wrela should still include fixed vector value types for kernels where exact
lanes matter.

```wrela
fn xor_block(a: Vec[16, U8], b: Vec[16, U8]) -> Vec[16, U8] {
    a ^ b
}
```

Fixed vectors are lower-level than tables and masks. They are appropriate for
building reusable kernels, implementing table operations, or writing code where
the desired lane shape is part of the algorithm.

Initial fixed vectors should target common 128-bit Advanced SIMD shapes:

- `Vec[16, U8]`
- `Vec[8, U16]`
- `Vec[4, U32]`
- `Vec[2, U64]`
- `Vec[4, F32]`
- `Vec[2, F64]`

## AArch64 Intrinsics And Assembly

Some kernels need exact instruction selection. Wrela should provide explicit
AArch64 intrinsic and assembly escape hatches, but they should not be the normal
vector programming model.

Example shape:

```wrela
use arch.aarch64.neon

fn checksum_step(input: Vec[16, U8]) -> Vec[8, U16]
    requires cpu.adv_simd
{
    neon.uaddlp(input)
}
```

Assembly and intrinsics must be explicit about:

- Required CPU features.
- Inputs and outputs.
- Clobbers.
- Memory effects.
- Whether the operation can trap.

This keeps sharp machine code available without making ordinary optimized code
depend on opaque assembly blocks.

## Physical Layout

`layout` declares a physical memory contract. It is used when outside hardware,
wire formats, disk formats, or ABIs require a specific representation.

```wrela
layout C data VirtioDescriptor {
    addr: U64
    len: U32
    flags: U16
    next: U16
}

layout packed data IpHeader {
    version_ihl: U8
    dscp_ecn: U8
    total_len: U16
    identification: U16
    flags_fragment: U16
    ttl: U8
    protocol: U8
    checksum: U16
    src: U32
    dst: U32
}

layout mmio data UartRegisters {
    data: Mmio[U32]
    status: Mmio[U32]
    control: Mmio[U32]
}
```

The split is:

- `data`: logical schema, compiler-owned bulk layout.
- `Table[data]`: columnar bulk storage.
- `layout C data`: physical C-like layout.
- `layout packed data`: packed wire or disk layout.
- `layout mmio data`: device register layout with special access rules.

Physical layout is opt-in. Logical data remains free for the compiler to store
and transform efficiently.

## Classes And Interfaces

`class` is for object, capability, adapter, and suite composition. Classes have
identity and carry dependencies.

```wrela
interface BlockDevice {
    fn read(index: U64, out: Buffer[U8]) -> Result[None, DiskError]
    fn write(index: U64, data: Buffer[U8]) -> Result[None, DiskError]
}

class VirtioBlockDevice implements BlockDevice {
    registers: unique VirtioBlockRegisters
    queue: unique VirtioQueue

    fn read(index: U64, out: Buffer[U8]) -> Result[None, DiskError] {
        queue.submit_read(index = index, out = out)
    }
}
```

Classes are not the default representation for bulk records. If there are many
items of the same logical shape, prefer `data` plus `Table`.

## Vectorization Diagnostics

Vectorization should be reviewable at build time. The compiler should be able to
explain when a table/mask operation lowered cleanly and when it did not.

Possible diagnostic modes:

```wrela
fn classify(packets: Table[Packet])
    vectorize diagnose
{
    let valid = packets.flags.has(PacketFlag.Valid)
    packets.flags[valid].set(PacketFlag.Checked)
}

fn classify_fast(packets: Table[Packet])
    vectorize require
{
    let large = packets.len > 1200
    packets.flags[large].set(PacketFlag.Jumbo)
}
```

If required vectorization fails, the compiler should report why:

```text
cannot vectorize classify_fast:
  call to timestamp_now has external effects
  packets.payload is a physical layout field with variable stride
```

This matches Wrela's general philosophy: important systems questions should be
answered by the compiler before the image boots.

## Non-Goals

This design does not require:

- Whole-program SIMD.
- Automatic vectorization of all loops.
- SVE in the initial language nucleus.
- Assembly for ordinary table operations.
- Array-of-structs layout for logical bulk data.
- Host-specific SIMD behavior hidden behind the test runner.

## Initial Language Shape

The first language nucleus should include:

- `module` and explicit `use` imports.
- `interface` for behavior contracts.
- `class` for capability-carrying objects and adapters.
- `data` for logical records.
- `Table[T]` for columnar bulk logical data.
- `Mask` for row selection and predication.
- `layout` for physical memory representation.
- `Vec[N, T]` for fixed vector kernels.
- `image` and `host image` roots.
- `test` declarations inside suite classes.

This gives Wrela a scalar authority model and a vector-friendly data model
without making either one masquerade as the other.

## Open Questions

The next design pass should settle:

- Exact `Table` capacity syntax and whether capacity is always static.
- Whether `Mask` is parameterized by capacity, table identity, or lane count.
- How table row iteration works without exposing row addresses.
- How table columns interact with ownership and borrowing.
- How masked writes report or forbid overlapping aliases.
- Whether `layout mmio` uses a distinct `Mmio[T]` field type or an enclosing
  layout rule.
- How `Vec[N, T]` values interact with ABI boundaries.
- How vectorization requirements are declared on functions, stages, or images.
- Whether Wrela should have first-class `pipeline` or `stage` declarations in
  addition to table/mask operations.
