# Remaining Language Decisions: Preferred Stance

Date: 2026-05-22

## Purpose

This document gives Codex's preferred default stance on the remaining Wrela
language decisions. It is intentionally opinionated. The goal is to create a
clear review target that can be edited, challenged, and folded back into the
main language design once the choices feel right.

The bias here is conservative for v1:

- Keep authority explicit.
- Prefer static compiler facts over runtime flexibility.
- Make memory and latency bounded by construction.
- Let table, mask, and loop shapes carry optimization intent.
- Avoid hidden allocation, hidden dispatch, hidden syscalls, and hidden
  dependency edges.

## 1. Constructors And Initialization

Preferred stance: classes use explicit class-scoped `constructor` declarations.
A constructor is not a top-level function and has no `self` receiver because
there is no object yet. It must return `Self(...)` with every field initialized
exactly once.

```wrela
class InMemoryBlockDevice<M: Memory> implements BlockDevice {
    memory: M
    blocks: U32
    storage: BlockArray

    constructor(memory: M, blocks: U32) {
        return Self(
            memory = memory,
            blocks = blocks,
            storage = BlockArray.allocate(memory = memory, blocks = blocks),
        )
    }

    fn read(mut self, index: U64, out: Buffer[U8]) -> Result[None, DiskError] {
        match index >= blocks {
            true => return Err(DiskError.OutOfRange)
            false => return storage.read(index = index, out = out)
        }
    }
}
```

Rules:

- A class constructor counts as class behavior, but a class that only wraps data
  and has no meaningful behavior should still be rejected as a value-like class.
- Data values use structural literals. Validation belongs in class methods or
  constructors that return `Result`.
- Constructor parameters move by default.
- A field declared `read T` must be initialized from an explicit `read` borrow.
- Partially initialized class values do not exist in source.
- Field default values should not exist in v1 except for compile-time constants
  on `data` values. Runtime defaults hide work and authority.
- Constructors may call methods on fully initialized dependencies, but not on
  `Self` before `Self(...)` has been returned.
- A failed recoverable constructor returns `Result[Self, E]`.
- A capacity or OOM failure during construction traps unless the constructed
  type explicitly models admission failure.

Review pressure point: whether constructors alone should satisfy the "class has
behavior" rule. My preference is yes for authority-bearing classes, no for
classes that are merely named bags of fields.

## 2. Borrowing And Lifetimes

Preferred stance: borrow checking should be mostly inferred, but the source
syntax for access authority stays explicit: `read`, `mut`, and `own`.

Rules:

- `read` allows shared access and cannot mutate or consume.
- `mut` allows exclusive access and can mutate owned state.
- `own` consumes the value.
- Any number of `read` borrows may coexist.
- A `mut` borrow excludes all other borrows for its duration.
- An owned value may be temporarily reborrowed as `read` or `mut`.
- Long-lived class fields are either owned or `read`; long-lived `mut` fields
  remain out of v1.
- Borrowed class fields cannot outlive the owner they borrow from.
- Values created from `with` frames carry frame lifetime and cannot escape that
  frame.
- Row tokens, scan indexes, drain items, and frame handles are scoped compiler
  capabilities. They cannot be stored, returned, published, or hidden in data.

I would avoid explicit lifetime parameters in v1 source unless we hit a wall.
The compiler can still report lifetimes in diagnostics using generated region
names:

```text
cannot store value from frame 'frame#2' into executor arena 'executor#0'
```

Returning borrowed views should be allowed only when the return type clearly
carries a lifetime from an input parameter. If that rule gets hard to explain,
v1 should disallow returning borrowed views and add it later.

## 3. Primitive Types And Scalar Semantics

Preferred stance: Wrela should use fixed-size scalar types and make dangerous
numeric behavior explicit.

Primitive scalar types:

- `Bool`: closed `true | false`.
- Unsigned integers: `U8`, `U16`, `U32`, `U64`, `USize`.
- Signed integers: `I8`, `I16`, `I32`, `I64`, `ISize`.
- Floats: `F32`, `F64`, included for data-plane kernels but not privileged in
  the core appliance model.
- `None`: unit value.
- Closed `enum` and `error` sums.
- Bitflags as a distinct declaration form or library type, not as ad hoc
  integer aliases.

Numeric rules:

- Default integer `+`, `-`, `*`, division, remainder, and shifts trap on
  overflow, division by zero, or invalid shift count.
- Explicit wrapping operations are available through named methods or operators
  such as `wrap_add`, `wrap_sub`, and `wrap_shl`.
- Saturating arithmetic is explicit.
- Checked arithmetic returns `Result` or `Option`.
- Widening conversions can use direct conversion syntax.
- Narrowing conversions must be explicit as trapping or checked conversion.
- Endian conversion is explicit. Wire and disk layouts should use wrapper types
  such as `Be[U16]` and `Le[U32]`.

`USize` and `ISize` are AArch64-sized. Since Wrela is AArch64-only, they are
64-bit, but the names are still useful for lengths, offsets, and ABI-shaped
values.

## 4. Pointers, Buffers, And Raw Memory

Preferred stance: ordinary Wrela code should not manipulate general raw
pointers. It should manipulate typed capabilities and bounded views.

Core memory-facing types:

- `Buffer[T]`: mutable bounded view of initialized contiguous elements.
- `ReadBuffer[T]`: read-only bounded view of initialized contiguous elements.
- `Bytes`: read-only byte view.
- `Address`: opaque virtual address value, not dereferenceable by itself.
- `PhysicalAddress`: opaque physical address value, usable only through
  platform or mapping authority.
- `Mmio[T]`: volatile memory-mapped register cell.
- `DmaBuffer[T]`: memory with DMA-suitable ownership, alignment, and cache
  policy.

Rules:

- No nullable pointers. Use `Option[T]`.
- No pointer arithmetic in ordinary code.
- Byte reinterpretation requires an explicit layout or parser.
- A `Buffer[T]` carries length, alignment, initialization state, and lifetime.
- Mutable buffer access requires exclusive borrow of the view.
- MMIO fields must be accessed through volatile operations with explicit memory
  ordering rules.
- Physical memory cannot be forged from integers. It comes from root/platform
  authority.

Low-level drivers can have an escape hatch, but it should be capability-shaped,
not ambient `unsafe`. For example, a platform authority may grant a
`RawMappingAuthority` to a page-table builder. That authority is explicit in
the image graph.

## 5. Construct-Level Effects

Preferred stance: Wrela should infer method effects and require annotations
only when a caller or optimization depends on them.

Effects are not a general runtime mechanism. They are compile-time facts used
for diagnostics, vectorization, test determinism, and image review.

Important inferred effects:

- `Trap`: can leave normal control flow through `trap`.
- `Io`: touches device or host IO capability.
- `Volatile`: performs MMIO or volatile memory access.
- `Time`: reads a clock.
- `Entropy`: reads randomness.
- `Arena`: consumes arena or frame capacity.
- `Mutate`: mutates owned state through `mut self` or a mutable capability.
- `Dma`: hands memory to a device.
- `Block`: may wait for an external event.

Examples of effect-sensitive rules:

- `vectorize require` rejects calls with `Io`, `Volatile`, `Time`, `Entropy`,
  or unknown mutation effects.
- Hosted deterministic tests can reject `Time` or `Entropy` unless passed fake
  capabilities.
- Image diagnostics can list methods that may `Block` in interrupt context.
- Pure table operations can be reordered only when effects permit it.

I would let users write constraints later, such as `requires effects none` or
`requires no_block`, but v1 can start with compiler-inferred diagnostics.

## 6. Tables, Masks, And Indexes

Preferred stance: table capacities are static const parameters in v1.

```wrela
Table[Packet, 256]
Index[PacketId, 512]
```

Rules:

- `Table[T, Rows]` requires `T: Data`.
- `Rows` is a `Const[U32]`.
- A table has logical length and static capacity.
- Insert past capacity traps unless the type explicitly models another policy.
- Table rows do not have stable memory addresses.
- Row tokens are scoped and cannot escape.
- Structural table operations that can move rows are forbidden while row tokens
  for that table are live.
- Stable identity is not a property of `Table`. If stable identity is needed,
  use an index or a future `StableTable` type.

Masks:

- A `Mask` carries table provenance and capacity. A mask produced from one table
  cannot be applied to another table accidentally.
- Masked writes must prove non-overlapping mutable access to the target
  columns.
- Mask combination is allowed only for compatible masks.

Indexes:

- Index strategy should be a type parameter or concrete index type, not a
  runtime policy field.
- An index maps keys to table row tokens or row IDs for one table.
- An index owns metadata, not row storage.
- Index insertion past capacity traps unless the index type explicitly models
  eviction or admission failure.

Preferred shape:

```wrela
unique class SessionStore<Rows: Const[U32], Slots: Const[U32]> {
    sessions: Table[Session, Rows]
    by_id: OpenAddressIndex[SessionId, Rows, Slots]

    fn find(read self, id: SessionId) -> Option[Session] {
        match by_id.find(key = id) {
            Some(row) => return Some(sessions[row])
            None => return None
        }
    }
}
```

## 7. Loop Edge Semantics

Preferred stance: loops are statement-shaped except for `reduce` and `scan`,
which produce values by design.

Rules:

- `break` is allowed in `repeat`, table-row `for`, `drain`, and `loop`.
- `continue` is allowed in `repeat`, table-row `for`, `drain`, and `loop`.
- `break` and `continue` are not allowed in `reduce` or `scan` v1.
- `reduce` uses `yield` to produce the next accumulator value.
- `scan` uses `until` as its only early-exit mechanism.
- `loop` is assumed non-terminating unless exited with `return`, `break`, or
  `trap`.
- Value-bearing `break` should not exist in v1.

Preferred reduction syntax:

```wrela
class PacketStats {
    fn total_length(read self, packets: Table[Packet, 256]) -> U64 {
        let valid = packets.flags.has(PacketFlag.Valid)

        let total = reduce packets.rows(valid) as row, acc: U64 = 0 {
            yield acc + packets.length[row]
        }

        return total
    }
}
```

Preferred scan result:

```wrela
enum Scan[I] {
    Found(index: I)
    Missing
}
```

`scan bytes as i until bytes[i] == 0 { ... }` returns `Scan[USize]` unless the
source chooses a narrower index type.

## 8. Generics Finishing Pass

Preferred stance: generics are monomorphized from the root image graph, with
capitalized constraints and explicit const parameters.

Built-in constraints:

- `Data`
- `Stored`
- `Copy`
- `Move`
- `Unique`
- `Const[T]`
- Static interface names such as `Console`, `Clock`, and `BlockDevice`

Rules:

- Type arguments are inferred at construction and method call sites when
  unambiguous.
- Explicit type arguments are allowed using square brackets:
  `RingBuffer[U8, 128](arena = arena)`.
- Const generics must be compile-time evaluable.
- No runtime generic dictionaries.
- No higher-kinded types in v1.
- No implicit trait objects or existential interface values.
- The compiler reports code and data footprint per instantiated type.
- Code sharing between compatible instantiations is allowed as an optimization
  only when it does not affect source semantics or diagnostics.

## 9. Layouts, ABI, And Calling Convention

Preferred stance: Wrela has one primary ABI target: AArch64 AAPCS64. Physical
layout is opt-in and source-visible.

Rules:

- `layout C data` follows the AArch64 C ABI for size, alignment, and field
  offsets.
- `layout packed data` has no implicit unaligned typed references. Field access
  lowers to safe loads and stores.
- `layout mmio data` requires `Mmio[T]` fields or an enclosing MMIO rule that
  makes every field volatile.
- Logical `data` has compiler-owned layout and is not ABI-stable.
- `Vec[N, T]` is not a stable external ABI type in v1 unless wrapped in an
  explicit layout or intrinsic boundary.
- Endian-specific physical fields use explicit wrapper types.
- Assembly and intrinsics must declare CPU features, inputs, outputs, clobbers,
  memory effects, and trap behavior.

C interop should be minimal in v1. The first goal is appliance images and
hosted tests, not a broad foreign-function interface.

## 10. Image Roots And Phases

Preferred stance: roots are composition declarations, not ordinary functions.
They are the only source of platform or host authority.

Root forms:

- `host image Name { phase run(host: unique MacOSHost) { ... } }`
- `image Name { phase boot(platform: unique QemuVirt) { ... } }`

Rules:

- Roots explicitly import every reachable module.
- Roots construct the authority graph.
- Roots narrow broad host/platform authority into smaller capabilities.
- Roots install fault policy.
- Roots select target CPU features and image/linker layout.
- Root phase declarations are not importable methods.
- Phase ordering is explicit.
- No module has initialization side effects outside the root graph.

Linker and image layout should be controlled by root-owned declarations, for
example named memory regions, sections, stacks, arenas, DMA regions, and boot
entry points.

## 11. Trap And Fault Payloads

Preferred stance: `trap` is abnormal control flow with type `Never`. It is not
recoverable and does not unwind through user code.

Preferred `TrapReport` shape:

```wrela
data TrapReport {
    code: TrapCode
    source: SourceLocation
    image: ImageName
    phase: PhaseName
    executor: Option[ExecutorId]
    message: StaticString
}
```

Rules:

- Trap report payloads must be bounded and allocation-free.
- Capacity violations, OOM, bounds failures, invalid shifts, integer overflow,
  failed `require`, and violated compiler checks trap by default.
- Hosted traps route to the hosted runtime, print a report, and cause failed
  process exit.
- Appliance traps route to the installed `FaultPolicy`.
- Without a fault policy, conservative behavior is halt the executor or image.
- A fault policy may log, halt, reboot, or isolate an executor, but it cannot
  resume the trapped computation.

Executor-local failure versus whole-image failure should be a root-owned policy
choice, not behavior hidden in libraries.

## 12. Test Assertions And Runner Results

Preferred stance: assertion failure is a test failure, not a trap. A trap inside
a test is reported separately as an abnormal failure.

Rules:

- `test` declarations are allowed only inside classes in v1.
- `assert expr` requires `expr: Bool`.
- Assertion failure records source location and expression text if available.
- `assert_eq(left, right)` can be a test helper method or compiler-known
  assertion form later, but plain `assert` is enough for v1.
- `with` fixtures are constructed fresh per test and destroyed after the test.
- Test-local fixture lifetimes cannot escape the test body.
- Timeouts are runner policy using an explicit `Clock` capability.
- Runner output uses explicit `Console` or result sinks.
- Hosted process exit mapping happens in the hosted root, not inside suites.

Heterogeneous generic suite lists should be represented by compiler-generated
static runner metadata for already-constructed suite values. This is not a
runtime interface object and does not create new source graph edges.

Preferred runner result:

```wrela
data TestSummary {
    passed: U32
    failed: U32
    trapped: U32
    timed_out: U32
}
```

## 13. Modules And Visibility

Preferred stance: modules are static namespaces with no initialization side
effects.

Rules:

- One module per source file for v1.
- Module path follows file path unless explicitly declared.
- `use` imports names explicitly.
- `pub` is required for cross-module visibility.
- Imports do not run code.
- Cyclic value dependencies are rejected.
- Cyclic type references are allowed only if they do not require impossible
  layout or initialization.
- Root images decide the reachable graph.
- Tests are reachable only through explicit root imports and construction.
- A package manifest can name source roots and target roots, but it cannot
  inject hidden dependencies.

I would avoid wildcard imports in v1. They make image review noisier and hide
authority edges.

## Suggested Decision Order

The next design passes should happen in this order:

1. Constructors and initialization.
2. Borrowing and lifetimes.
3. Primitive types and scalar semantics.
4. Pointers, buffers, and raw memory.
5. Tables, masks, and indexes.
6. Loop edge semantics.
7. Effects.
8. Layouts, ABI, and image roots.
9. Test assertion and runner result details.
10. Modules and visibility.

The first four are the foundation. Once they are settled, the remaining
decisions should get much easier to specify without hidden contradictions.
