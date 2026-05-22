# Wrela Language And Test Design

Date: 2026-05-22

## Purpose

Wrela is an AArch64-only systems language for building complete appliance
images. The language should make machine authority, memory ownership, data
layout, test execution, and hot data paths visible to the compiler without
turning all code into assembly or compiler folklore.

This design captures the initial shape of the language nucleus:

- Scalar control and authority code remains explicit and readable.
- Modules do not allow top-level `fn` declarations.
- Modules are static namespaces with no initialization side effects.
- Callable functions are methods under classes.
- Interfaces are static compile-time contracts, not runtime vtables.
- Constructors initialize every class field exactly once.
- Class fields are immutable after construction.
- Class values move by default; shared dependencies must be `read`.
- Borrowing uses explicit `read`, `mut`, and `own` access authority.
- Generics are compile-time only and use capitalized constraints.
- Interface names are static constraints, not hidden runtime field types.
- Scalar primitive types are fixed-size and dangerous numeric behavior is
  explicit.
- Scalar branching uses exhaustive `match`, not a generic `if`.
- Loops describe work shape through `repeat`, table `for`, `drain`, `reduce`,
  `scan`, and intentional `loop`.
- Methods and phase blocks use explicit `return`.
- Recoverable errors are typed values, not exceptions.
- Privileged machine effects come from `unique class` authority, not
  developer-written permission annotations.
- Memory is explicit authority, not an ambient allocator.
- OOM and capacity violations trap by default.
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
- `assert value` and `assert same` for value equality versus identity claims.
- `with` fixtures for test-local fake construction.
- Explicit root images that import and construct suite classes.

This document defines the language shape and the explicit hosted/QEMU test model
as one integrated design.

## No Top-Level Functions

Wrela modules do not allow top-level `fn` declarations. A module can declare
types, interfaces, classes, errors, layouts, imports, and image roots, but
ordinary callable behavior lives under classes.

This keeps executable behavior attached to an explicit owner:

- Capability behavior lives on capability classes.
- Adapter behavior lives on adapter classes.
- Data-plane kernels live on kernel or service classes.
- Test behavior lives inside test suite classes.
- Interfaces declare method requirements, but do not define free functions.

Image and host-image roots may contain phase or entry declarations. Those are
root composition hooks, not importable module-level functions. They establish
authority and construct classes that do the ordinary work.

## Modules And Visibility

Modules are static namespaces. Importing a module does not run code, allocate
memory, install handlers, register tests, or change the image graph except by
making names available to the importing module.

Initial module rules:

- One module per source file.
- The module path follows the file path unless explicitly declared.
- `use` imports names explicitly.
- `pub` is required for cross-module visibility.
- Wildcard imports should not exist in v1.
- Cyclic value dependencies are rejected.
- Cyclic type references are allowed only when they do not require impossible
  layout or initialization.
- Root images decide the reachable graph.
- Tests are reachable only through explicit root imports and construction.
- A package manifest can name source roots and target roots, but it cannot
  inject hidden dependencies.

This keeps image review honest: dependency edges are visible in source and no
module can smuggle initialization work into the image.

## Types And Ownership

Wrela's type system separates value shape, behavior, and ownership.

The core categories are:

- `data`: plain logical values. Data is copyable when all fields are copyable.
- `layout data`: physical representation for ABI, wire, disk, and MMIO
  boundaries.
- `class`: behavior plus owned dependencies. A class must define at least one
  method or test declaration.
- `unique class`: authority-bearing behavior and state with stricter
  construction and graph checks.
- `interface`: static method contract.

If a type has fields but no methods or test declarations, it should be `data`,
not `class`.

Class values move by default. They are not implicitly copied. If two owners need
the same behavior, the image or constructor code must create two instances and
move one into each owner.

```wrela
class HeaderParser {
    limits: ParserLimits

    fn parse(read self, bytes: Bytes) -> Result[Header, HeaderError] {
        return HeaderParserCore(limits = limits).parse(bytes = bytes)
    }
}

class ServiceA {
    parser: HeaderParser

    fn run(read self, bytes: Bytes) -> Result[Header, HeaderError] {
        return parser.parse(bytes = bytes)
    }
}

class ServiceB {
    parser: HeaderParser

    fn run(read self, bytes: Bytes) -> Result[Header, HeaderError] {
        return parser.parse(bytes = bytes)
    }
}

host image ParserServices {
    phase run(host: unique MacOSHost) {
        let service_a = ServiceA(parser = HeaderParser(limits = limits_a))
        let service_b = ServiceB(parser = HeaderParser(limits = limits_b))

        return None
    }
}
```

Reusing a moved class value is invalid:

```wrela
host image InvalidParserReuse {
    phase run(host: unique MacOSHost) {
        let parser = HeaderParser(limits = limits)
        let service_a = ServiceA(parser = parser)
        let service_b = ServiceB(parser = parser) // invalid: parser was moved

        return None
    }
}
```

Shared dependencies must be explicit read-only borrows:

```wrela
class SharedServiceA {
    parser: read HeaderParser

    fn run(read self, bytes: Bytes) -> Result[Header, HeaderError] {
        return parser.parse(bytes = bytes)
    }
}

class SharedServiceB {
    parser: read HeaderParser

    fn run(read self, bytes: Bytes) -> Result[Header, HeaderError] {
        return parser.parse(bytes = bytes)
    }
}

host image SharedParserServices {
    phase run(host: unique MacOSHost) {
        let parser = HeaderParser(limits = limits)
        let service_a = SharedServiceA(parser = read parser)
        let service_b = SharedServiceB(parser = read parser)

        return None
    }
}
```

The compiler checks that read-borrowed fields cannot mutate or consume the
borrowed object and cannot outlive the owner they borrow from.

## Class Fields And Self Modes

Class fields are immutable bindings after construction. A method may mutate the
state behind an owned field when it has `mut self`, but it may not reassign the
field itself.

```wrela
unique class SessionStore {
    sessions: Table[Session, 4096]
    by_id: Index[SessionId, 8192]

    fn insert(mut self, session: Session) -> None {
        let row = sessions.insert(session)
        by_id.insert(key = session.id, row = row)

        return None
    }

    fn replace_index(mut self, index: Index[SessionId, 8192]) -> None {
        self.by_id = index // invalid: class fields are immutable bindings
    }
}
```

Methods explicitly declare their receiver mode:

- `read self`: shared read access, no mutation, no consumption.
- `mut self`: exclusive mutable access to owned state.
- `own self`: consumes the object.

Long-lived class fields may be owned or `read`. Initial Wrela should not allow
long-lived `mut` fields; shared mutable dependencies should be modeled through a
unique owner/coordinator or an explicit synchronization capability.

```wrela
unique class Coordinator {
    store: SessionStore

    fn mark(mut self, id: SessionId, now: Tick) -> None {
        store.mark_active(id = id, now = now)

        return None
    }
}
```

This keeps the dependency graph stable after construction while still allowing
owned state to change through explicit mutable receiver access.

## Constructors And Initialization

Classes use explicit class-scoped `constructor` declarations. A constructor is
not a top-level function and has no `self` receiver because there is no object
yet. It returns `Self(...)` with every field initialized exactly once.

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

Constructor rules:

- Constructor parameters move by default.
- A field declared `read T` must be initialized from an explicit `read` borrow.
- Partially initialized class values do not exist in source.
- Runtime field defaults should not exist in v1. Runtime defaults hide work and
  authority.
- Data values use structural literals. Validation belongs in class constructors
  or methods that return `Result`.
- Constructors may call methods on fully initialized dependencies, but not on
  `Self` before `Self(...)` has been returned.
- A recoverably failing constructor returns `Result[Self, E]`.
- Capacity or OOM failure during construction traps unless the constructed type
  explicitly models admission failure.

A constructor can count as class behavior for authority-bearing classes, but it
should not turn a value-like bag of fields into a class. If a type has no
meaningful behavior or authority, it should be `data`.

## Borrowing And Lifetimes

Borrow checking should be mostly inferred, but source syntax for access
authority stays explicit.

Borrow rules:

- `read` allows shared access and cannot mutate or consume.
- `mut` allows exclusive access and can mutate owned state.
- `own` consumes the value.
- Any number of `read` borrows may coexist.
- A `mut` borrow excludes all other borrows for its duration.
- An owned value may be temporarily reborrowed as `read` or `mut`.
- Borrowed class fields cannot outlive the owner they borrow from.
- Values created from `with` frames carry frame lifetime and cannot escape that
  frame.
- Row tokens, scan indexes, drain items, and frame handles are scoped compiler
  capabilities. They cannot be stored, returned, published, or hidden in data.

Long-lived class fields remain owned or `read` in v1. Long-lived `mut` fields
stay out of the initial language.

Wrela should avoid explicit lifetime parameters in v1 source unless the simpler
rules prove insufficient. Diagnostics can still name compiler-generated
regions:

```text
cannot store value from frame 'frame#2' into executor arena 'executor#0'
```

Returning borrowed views should be allowed only when the return type clearly
carries a lifetime from an input parameter. If that rule becomes hard to explain
or diagnose, v1 should disallow returning borrowed views and add them later.

## Primitive Types And Scalar Semantics

Primitive scalar types are fixed-size and source-visible:

- `Bool`: closed `true | false`.
- Unsigned integers: `U8`, `U16`, `U32`, `U64`, `USize`.
- Signed integers: `I8`, `I16`, `I32`, `I64`, `ISize`.
- Floats: `F32`, `F64`, primarily for data-plane kernels.
- `None`: unit value.
- Closed `enum` and `error` sums.
- Bitflags as a distinct declaration form or library type, not ad hoc integer
  aliases.

`USize` and `ISize` are 64-bit because Wrela is AArch64-only. The names remain
useful for lengths, offsets, and ABI-shaped values.

Default integer arithmetic traps on overflow, division by zero, invalid
remainder, and invalid shift counts. Wrapping, saturating, and checked
arithmetic are explicit operations.

Conversion rules:

- Widening conversions can use direct conversion syntax.
- Narrowing conversions must be explicit as trapping or checked conversion.
- Endian conversion is explicit.
- Wire and disk layouts should use wrapper types such as `Be[U16]` and
  `Le[U32]`.

## Unique Classes

`unique class` is for authority-bearing owners and identity-sensitive state.

Examples include:

- host and platform roots
- hardware authorities
- memory arenas
- executor state
- drivers and device paths
- queues, topics, caches, tables, and indexes that own durable storage
- DMA and MMIO capabilities

Unique values move by default and cannot be copied. They can be borrowed with
`read` or `mut` only when the capability type permits that access. Most root
authorities should be narrowed into smaller capabilities before being shared.

Unique classes also participate in image graph checks: roots cannot be forged,
memory cannot be assigned to the wrong executor, device paths cannot be passed
to multiple owners, and authority-bearing values cannot be hidden inside
copyable data.

## Authority-Gated Machine Effects

Privileged behavior comes from authority in the object graph, not from
developer-written permission annotations. A method cannot opt into IO, raw
memory, pointer arithmetic, assembly, MMIO, DMA, blocking, entropy, or time by
writing a `requires` effect clause.

Instead, those effects are available only through `unique class` capabilities
granted by roots or by other unique authorities.

```wrela
unique class PageTableBuilder {
    memory: unique PhysicalMemoryAuthority

    asm fn invalidate_tlb(mut self, address: Address) -> None {
        // privileged instruction selected by this authority-bearing class
    }

    fn map(mut self, physical: PhysicalAddress, virtual: Address) -> None {
        // address arithmetic is permitted because this class owns mapping
        // authority.
        memory.map(physical = physical, virtual = virtual)

        return None
    }
}
```

The compiler still infers and reports effects:

```text
PageTableBuilder.map:
  effects: PhysicalMemory, AddressArithmetic, Trap
  authority source: image QemuVirtBoot.platform.memory
```

Important inferred effects include:

- `Trap`: can leave normal control flow through `trap`.
- `Io`: touches device or host IO capability.
- `Volatile`: performs MMIO or volatile memory access.
- `Time`: reads a clock.
- `Entropy`: reads randomness.
- `Arena`: consumes arena or frame capacity.
- `Mutate`: mutates owned state through `mut self` or a mutable capability.
- `Dma`: hands memory to a device.
- `Block`: may wait for an external event.
- `AddressArithmetic`: manipulates raw addresses or pointer-shaped values.
- `Assembly`: enters an assembly function.

Effect-sensitive compiler checks use those inferred facts. For example,
`vectorize require` rejects calls with IO, volatile, time, entropy, or unknown
mutation effects. Hosted deterministic tests can reject time or entropy unless
the root passes fake capabilities. Image diagnostics can list methods that may
block in interrupt context.

But developers do not write effect annotations as permission slips. If normal
code needs privileged work, it must receive a narrowed explicit capability from
the image graph.

## Static Interfaces

Interfaces are static contracts. They do not imply runtime vtables, fat
pointers, dynamic dispatch, hidden allocation, or runtime type dictionaries.

```wrela
interface BlockDevice {
    fn read(mut self, index: U64, out: Buffer[U8]) -> Result[None, DiskError]
    fn write(mut self, index: U64, data: Buffer[U8]) -> Result[None, DiskError]
}
```

A class satisfies an interface by providing the required methods. Calls through
interface-constrained generics are statically resolved or monomorphized.
Receiver mode is part of the contract.

An interface name is not a default runtime type. Fields and parameters must be
concrete types or generic type parameters constrained by interfaces.

```wrela
class BadLoader {
    disk: BlockDevice // invalid: would require a hidden runtime interface value

    fn read_block(mut self, index: U64, out: Buffer[U8]) -> Result[None, DiskError] {
        return disk.read(index = index, out = out)
    }
}

class Loader<D: BlockDevice> {
    disk: D

    fn read_block(mut self, index: U64, out: Buffer[U8]) -> Result[None, DiskError] {
        return disk.read(index = index, out = out)
    }
}
```

Dynamic dispatch, if Wrela ever needs it, must be an explicit value such as a
declared dispatch table capability. It is not the default meaning of
`interface`.

## Generics And Static Interfaces

Wrela generics are compile-time parameters. They should make reuse possible
without adding runtime dictionaries, hidden type metadata, implicit allocation,
or dynamic dispatch.

The initial generic parameter kinds are:

- Type parameters: `T: Data`, `D: BlockDevice`, `C: Console`.
- Const parameters: `Rows: Const[U32]`, `Bytes: Const[U64]`.
- Capacity parameters as ordinary const parameters used by tables, indexes,
  rings, queues, and arenas.

Constraints use capitalized names. Built-in constraints should start small:

- `Copy`: values can be copied implicitly.
- `Move`: values can be moved and consumed.
- `Data`: logical record values suitable for table storage.
- `Stored`: values with compiler-known storage requirements.
- `Unique`: authority-bearing values with graph checks.
- Interface names such as `BlockDevice`, `Clock`, or `Console`.
- `Const[T]`: compile-time constant values of scalar type `T`.

Example:

```wrela
interface HeaderParsing {
    fn parse(read self, bytes: Buffer[U8]) -> Result[Header, HeaderError]
}

class HeaderLoader<D: BlockDevice, P: HeaderParsing> {
    disk: D
    parser: P

    fn load(mut self, out: Buffer[U8]) -> Result[Header, LoadError] {
        try disk.read(index = 0, out = out) else return LoadError.Disk

        let header = try parser.parse(bytes = out) else return LoadError.Parse

        return Ok(header)
    }
}
```

`HeaderLoader` has no interface-typed fields. A root image constructs it with
concrete values, and the compiler specializes the used instance.

Capacity-shaped types use const generics:

```wrela
unique class SessionStore<Rows: Const[U32], IndexSlots: Const[U32]> {
    sessions: Table[Session, Rows]
    by_id: Index[SessionId, IndexSlots]

    fn insert(mut self, session: Session) -> None {
        let row = sessions.insert(session)
        by_id.insert(key = session.id, row = row)

        return None
    }
}
```

Generic methods are allowed, but they are still class methods and still require
an explicit receiver:

```wrela
class ValueOps {
    fn identity<T: Move>(read self, value: T) -> T {
        return value
    }

    fn copy<T: Copy>(read self, value: T) -> T {
        return value
    }
}
```

Because Wrela compiles from root images downward, every generic instantiation is
known before final code generation. The compiler can monomorphize or otherwise
specialize those instances and report their code and data footprint. Sharing
machine code between compatible instantiations can be a later optimization, but
the source semantics should not depend on it.

Type arguments are inferred at construction and method call sites when
unambiguous. Explicit type arguments use square brackets:
`RingBuffer[U8, 128](arena = arena)`.

Const generics must be compile-time evaluable. Wrela should not include
higher-kinded types, runtime generic dictionaries, or existential interface
objects in v1.

Tables should initially require `T: Data`, not arbitrary classes. This
preserves the columnar storage and vectorization model. Classes can own tables
and indexes, but a table should not become a bag of hidden object identities.

## AArch64-Only Backend

Wrela targets AArch64 only. That lets the language and compiler assume:

- AArch64 calling convention and register families.
- AArch64 memory ordering rules.
- AArch64 exception levels and boot realities.
- Advanced SIMD/NEON as the first vector target when available.
- Optional future SVE support behind platform and image target selection.

The language should still avoid baking a single microarchitecture into source
semantics. CPU features belong to the root-selected platform target and the
unique platform authority, not ambient compiler assumptions or method-level
permission annotations.

The initial hardware target order is:

1. QEMU `virt` generic AArch64 machine.
2. Raspberry Pi 5.
3. GCP cloud ARM VM.

Each target should expose a concrete platform authority:

```wrela
unique class QemuVirtPlatform
unique class RaspberryPi5Platform
unique class GcpArmVmPlatform
```

Portable code should sit behind narrowed interfaces such as `Console`, `Clock`,
`Memory`, `BlockDevice`, `NetworkDevice`, and platform-specific device
capabilities. Boot code can be target-specific; ordinary services should not
need to know which platform root created their capabilities.

Example:

```wrela
image PacketAppliance target QemuVirtPlatform {
    phase boot(platform: unique QemuVirtPlatform) {
        let console = UartConsole(platform.uart0.claim())
        console.write("packet appliance booted")

        return None
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

## Scalar Control Flow

Wrela should not have a generic `if` statement in the core language. Scalar
branching is expressed with exhaustive `match`.

`Bool` is a closed two-case type:

```wrela
class ExecutorChooser {
    fn choose(read self, ready: Bool, fast: Executor, idle: Executor) -> Executor {
        match ready {
            true => return fast
            false => return idle
        }
    }
}
```

Closed sums must handle every case:

```wrela
class DeviceStatusHandler {
    fn handle(read self, status: DeviceStatus) -> Result[None, DeviceError] {
        match status {
            DeviceStatus.Ready => return Ok(None)
            DeviceStatus.Busy => return Err(DeviceError.Retry)
            DeviceStatus.Gone => return Err(DeviceError.DeviceGone)
        }
    }
}
```

Open numeric domains can use ranges and a fallback arm when the input space is
not statically enumerable:

```wrela
class DeviceStatusDecoder {
    fn decode(read self, raw: U32) -> DeviceStatus {
        match raw {
            0 => return DeviceStatus.Ready
            1 => return DeviceStatus.Busy
            2..=15 => return DeviceStatus.Recoverable(raw)
            _ => return DeviceStatus.Unknown(raw)
        }
    }
}
```

The split is:

- `match` handles scalar and stateful control flow.
- `Mask` handles table-wide data-parallel control flow.

This keeps state handling exhaustive while leaving vectorizable bulk decisions
in the table/mask model.

## Loop Shapes

Loops should describe the shape of work, not merely spell "jump backward until a
mutable condition changes." Wrela should avoid a generic `while` in the initial
language nucleus. Instead, each loop form tells the compiler what kind of work
is happening.

The initial loop forms are:

- `repeat N as i`: finite counted work.
- `for row in table.rows(mask)`: finite table-row work over compiler-owned row
  tokens.
- `drain queue.up_to(N) as item`: finite systems batch work from a queue, ring,
  topic, or ready list.
- `reduce rows as row, acc { body }`: finite accumulation over rows, bytes, or
  other bounded ranges.
- `scan bytes as i until predicate { body }`: bounded sentinel search over a
  contiguous byte range.
- `loop`: intentional unbounded control flow.

This gives the compiler stronger facts than a traditional `for` loop:

- Known trip counts or upper bounds.
- Bounded latency for interrupt and executor work.
- Valid index or row-token ranges.
- Row tokens that cannot escape or become pointers.
- Reduction operations that can be checked before reordering or vectorizing.
- Sentinel scans that can lower to scalar, NEON, or target-specific search
  kernels.
- Clear separation between finite work and intentional event loops.

### Counted Loops

`repeat` is for finite counted work. The count is evaluated before the loop and
cannot be mutated by the body.

```wrela
class BufferFill {
    fn zero(read self, bytes: Buffer[U8]) -> None {
        repeat bytes.len as i {
            bytes[i] = 0
        }

        return None
    }
}
```

When the count is a constant or capacity generic, the compiler can unroll,
peel, vectorize, and remove redundant bounds checks. Nested `repeat` loops have
explicit product bounds, which also helps frame-memory and latency analysis.

### Table Row Loops

Table loops iterate over logical rows without exposing row addresses. The loop
variable is a row token owned by the compiler. It can index columns in the table
it came from, but it cannot be stored, returned, published, or converted to a
pointer.

```wrela
class TimerTableOps {
    fn mark_ready_rows(read self, timers: Table[TimerEntry, 4096], now: Tick) -> None {
        let expired = timers.deadline <= now

        for row in timers.rows(expired) {
            timers.state[row] = TimerState.Ready
        }

        return None
    }
}
```

For simple column operations, direct mask assignment remains preferable:
`timers.state[expired] = TimerState.Ready`. Row loops are the imperative escape
hatch when each selected row needs more work.

### Drain Loops

`drain` is for bounded systems batches. It removes or claims up to a declared
number of items from a source and runs the body for each item actually obtained.

```wrela
interface PacketHandler {
    fn handle(mut self, packet: Packet) -> None
}

class PacketPump<H: PacketHandler> {
    queue: PacketQueue
    handler: H

    fn poll(mut self) -> None {
        drain queue.up_to(64) as packet {
            handler.handle(packet = packet)
        }

        return None
    }
}
```

The bound is part of the program's latency contract. The compiler and image
diagnostics can reason about worst-case work per poll, interrupt, executor tick,
or hosted test step.

### Reductions

`reduce` is for bounded accumulation. The source is a finite range such as table
rows, bytes, lanes, or a bounded batch. The accumulator is explicit, and the
compiler may only reorder or vectorize the reduction when the accumulator
operation's semantics allow it.

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

For table-row reductions, the selected row token is available as `row` inside
the block. For byte reductions, the index token is available under the name
chosen by the source form.

`reduce` uses `yield` to produce the next accumulator value. `break` and
`continue` are not valid inside `reduce` in v1.

Integer wrapping addition, bitwise operations, min/max, count, any, and all are
good initial reduction targets. Floating-point and saturating arithmetic should
not be reassociated unless their types or operations explicitly permit that.

### Scans

`scan` is for bounded sentinel search. It walks a finite byte range in order,
stops when the `until` predicate is true, and returns a closed result that
distinguishes found from missing.

```wrela
interface ByteScanSink {
    fn observe(mut self, byte: U8) -> None
}

class ByteScanner<S: ByteScanSink> {
    sink: S

    fn find_nul(mut self, bytes: Buffer[U8]) -> Option[U32] {
        let found = scan bytes as i until bytes[i] == 0 {
            sink.observe(byte = bytes[i])
        }

        match found {
            Scan.Found(i) => return Some(i)
            Scan.Missing => return None
        }
    }
}
```

The body runs for elements that have not satisfied the predicate. Pure scans can
lower to vectorized compare/search kernels. Scans with side effects remain
bounded and analyzable, but the effects constrain reordering.

`scan` uses `until` as its only early-exit mechanism in v1. `break` and
`continue` are not valid inside `scan`.

### Intentional Control Loops

`loop` is the explicit unbounded form. It is appropriate for event loops,
driver state machines, executor dispatch, and appliance control flow.

```wrela
class ExecutorLoop<D: Dispatcher> {
    dispatcher: D
    executor: Executor

    fn run(mut self) -> Never {
        loop {
            let event = executor.next()
            dispatcher.handle(event = event)
        }
    }
}
```

The compiler should not try to prove that `loop` terminates. It should instead
treat it as intentional non-termination unless the body exits with `return`,
`break`, or `trap`. Bounded retry and polling should usually be expressed with
`repeat Attempts as attempt`, not open-ended `loop`.

`break` and `continue` are valid in `repeat`, table-row `for`, `drain`, and
`loop`. Value-bearing `break` should not exist in v1.

## Return

`return` is a keyword and should be used explicitly in methods, phases, and
expanded error handlers. Wrela should not rely on implicit final-expression
returns.

```wrela
class Math {
    fn add(read self, a: U32, b: U32) -> U32 {
        return a + b
    }

    fn mark_seen(read self, flags: Flags) -> Flags {
        return flags.set(Flag.Seen)
    }
}
```

Methods that produce no useful value return `None` explicitly:

```wrela
class BannerWriter<C: Console> {
    console: C

    fn write(mut self) -> None {
        console.write("ready")

        return None
    }
}
```

## Error Values

Recoverable failures are values. Wrela should not have exceptions, hidden
unwinding, implicit process exit, or ambient panic behavior.

The core recoverable forms are:

- `Option[T]` for absence.
- `Result[T, E]` for expected failure.
- Closed `error` sums for typed failure domains.

Example:

```wrela
error DiskError {
    Timeout
    BadBlock(index: U64)
    DeviceGone
}

error HeaderError {
    BadMagic
    UnsupportedVersion(version: U16)
    Truncated
}

error LoadError {
    Disk(error: DiskError)
    Parse(error: HeaderError)
    BadMagic
    UnsupportedVersion(version: U16)
    Truncated
}
```

Callers handle errors with exhaustive `match` when policy differs by case:

```wrela
class RequiredBlockReader<D: BlockDevice> {
    disk: D

    fn read(mut self, index: U64, out: Buffer[U8]) -> Result[None, LoadError] {
        match disk.read(index, out) {
            Ok(_) => return Ok(None)
            Err(DiskError.Timeout) => return Err(LoadError.Disk(DiskError.Timeout))
            Err(DiskError.BadBlock(block)) => return Err(LoadError.Disk(DiskError.BadBlock(block)))
            Err(DiskError.DeviceGone) => return Err(LoadError.Disk(DiskError.DeviceGone))
        }
    }
}
```

Adding a new `DiskError` case should force relevant matches to update.

## Try Else

`try` is explicit early-return sugar over `Result`. It is not an exception and
does not unwind.

When the source error type matches the enclosing method's error type, plain
`try` is valid:

```wrela
interface DiskFlush {
    fn flush(mut self) -> Result[None, DiskError]
}

class DiskFlusher<D: DiskFlush> {
    disk: D

    fn flush_all(mut self) -> Result[None, DiskError] {
        try disk.flush()

        return Ok(None)
    }
}
```

The inline mapped form uses `else return` so the control flow is visible:

```wrela
class HeaderLoader<D: BlockDevice> {
    disk: D
    parser: HeaderParser

    fn load(mut self, out: Buffer[U8]) -> Result[Header, LoadError] {
        try disk.read(0, out) else return LoadError.Disk

        let header = try parser.parse(out) else return LoadError.Parse

        return Ok(header)
    }
}
```

The inline mapped form means:

```text
on Ok(value), evaluate to value
on Err(err), return Err(Constructor(err)) from the current method
```

The constructor in `else return Constructor` must accept the source error. If a
caller wants to discard or inspect the source error, it must use the expanded
form.

The expanded form binds the source error and requires the block to diverge with
`return`, `trap`, or another `Never`-returning expression:

```wrela
class CheckedHeaderLoader {
    parser: HeaderParser

    fn load(read self, bytes: Buffer[U8]) -> Result[Header, LoadError] {
        let header = try parser.parse(bytes) else err {
            match err {
                HeaderError.BadMagic => return Err(LoadError.BadMagic)
                HeaderError.UnsupportedVersion(version) => {
                    return Err(LoadError.UnsupportedVersion(version))
                }
                HeaderError.Truncated => return Err(LoadError.Truncated)
            }
        }

        return Ok(header)
    }
}
```

This gives Wrela three levels of error handling:

- `match` for full policy.
- `try expr` for same-error propagation.
- `try expr else return Constructor` or `try expr else err { ... }` for explicit
  mapping.

## Trap And Fault Policy

A `trap` is not a recoverable error. It is an explicit transition out of normal
program semantics and has type `Never`.

Use traps for:

- Violated invariants.
- Impossible states.
- Bounds failures that cannot be represented as `Result`.
- Security stops.
- Compiler-inserted checks whose failure means normal execution cannot
  continue.

Example:

```wrela
class TrustedHeaderParser {
    parser: HeaderParser

    fn parse(read self, bytes: Buffer[U8]) -> Header {
        match parser.parse(bytes) {
            Ok(header) => return header
            Err(HeaderError.BadMagic) => trap("trusted header parser failed: bad magic")
            Err(HeaderError.UnsupportedVersion(_)) => trap("trusted header parser failed: version")
            Err(HeaderError.Truncated) => trap("trusted header parser failed: truncated")
        }
    }
}
```

Hosted execution should route traps to the hosted test/runtime trap handler,
report source location and reason, and exit the process with failure.

Appliance execution should route traps through an image-installed fault policy
when one exists. Without an installed policy, the conservative behavior is to
halt the current executor or image.

Fault policy is root-owned authority:

```wrela
interface FaultPolicy {
    fn fatal(mut self, reason: TrapReport) -> Never
}

image QemuTests target QemuVirtPlatform {
    phase boot(platform: unique QemuVirtPlatform) {
        let console = UartConsole(platform.uart0.claim())
        let runtime = platform.runtime.claim()
        let faults = SerialFaultPolicy(console = console)
        let runner = TestRunner(console = console)

        runtime.install_fault_policy(faults)

        runner.run([])

        return None
    }
}
```

Trap reports must be bounded and allocation-free:

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

Traps, hardware exceptions, and `Result` errors are distinct:

- `Result` is expected and typed.
- `trap` is deliberate abnormal termination of normal semantics.
- Hardware exceptions are machine events that may be reported through fault
  policy when the platform can do so.

## Memory Authority

Wrela should not have ambient allocation. There is no default heap, no ordinary
`new`, no garbage collector, no general-purpose free, and no library path that
secretly grows memory.

Memory is an explicit authority graph:

- Host roots receive host-backed memory authority.
- Appliance roots receive firmware/platform-derived physical memory authority.
- Root arenas are named, bounded, and created from that authority.
- Child arenas are explicitly claimed for executors, drivers, topics, queues,
  tables, indexes, caches, DMA buffers, and scratch spaces.
- Classes that need memory receive a memory capability through construction or
  method parameters.
- Ordinary modules cannot forge physical regions, arenas, byte views, or raw
  memory authority from integers.

Example root shape:

```wrela
image PacketAppliance target QemuVirtPlatform {
    phase boot(platform: unique QemuVirtPlatform) {
        let region = platform.memory.require_region(
            name = "root",
            bytes = 64 * MiB,
            align = 4096,
        )

        let root = region.create_arena(identity = "packet.root")
        let rx_memory = root.child(identity = "rx", bytes = 16 * MiB, align = 4096)
        let worker_memory = root.child(identity = "worker", bytes = 16 * MiB, align = 4096)

        let worker = PacketWorker(memory = worker_memory)

        return platform.vcpu0.enter(worker)
    }
}
```

The compiler should be able to report the memory authority tree, including
root regions, child arenas, executor ownership, queue/topic buffers, tables,
indexes, caches, DMA-intended buffers, and scratch frame bounds.

## Pointers, Buffers, And Raw Memory

Ordinary Wrela code should not manipulate general raw pointers. It should
manipulate typed capabilities and bounded views.

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

Pointer arithmetic, raw address manipulation, and assembly access are allowed
only inside `unique class` capabilities that own the relevant machine
authority. This keeps low-level drivers possible without creating ambient
machine access.

## Durable Arenas And Frames

Wrela separates durable memory from temporary frame memory.

Durable arenas hold state that survives events:

- executor state
- driver state
- topic and queue buffers
- tables and indexes
- caches
- DMA buffers

Frame memory is bounded scratch:

```wrela
class PacketWorker {
    memory: ExecutorArena

    fn tick(mut self, input: PacketBatch) -> None {
        with memory.frame(bytes = 64 * KiB, align = 64) as frame {
            let decoded = frame.place(DecodedBatch(input = input))
            let scratch = frame.reserve(bytes = 4096, align = 64)

            PacketDecoder(frame = frame).decode(batch = decoded, scratch = scratch)
        }

        return None
    }
}
```

Values created from a frame carry that frame lifetime. A frame-backed value
cannot be:

- returned from the method
- stored into longer-lived state
- assigned to a variable declared outside the frame
- published to a topic or interrupt queue
- captured by an executor, driver, path, cache, or shared region
- stored in a sibling or parent frame

Parent-lifetime values can be read inside child frames. Child-lifetime values
cannot be stored into parent-lifetime values. The rule is lifetime-based, not
name-based; aliases carry the same hidden lifetime.

## Infallible Bounded Memory

Default memory operations are infallible in source and trap on capacity
violation.

Examples:

```wrela
class SessionStore {
    sessions: Table[Session, 4096]
    by_id: Index[SessionId, 8192]

    fn insert(mut self, session: Session) -> None {
        let row = sessions.insert(session)
        by_id.insert(key = session.id, row = row)

        return None
    }
}
```

If `sessions` is full, or `by_id` cannot insert within its bounded policy, the
operation traps. This is intentional: for ordinary durable memory, OOM means the
image memory plan or input contract is wrong.

Non-trapping capacity behavior must be explicit in the type or policy name:

- `EvictingCache`
- `DroppingRing`
- `LossyTopic`
- bounded application containers that explicitly model admission refusal

Cache-full is not ordinary OOM when the cache is declared as evicting. Queue
overflow is not ordinary OOM when the queue is declared lossy or dropping.
Those are domain policies, not hidden allocation failures. Ordinary table,
index, arena, frame, and ring capacity violations trap by default.

## Tables And Indexes

Wrela should not provide a built-in unbounded `HashMap`. Hash maps combine
ambient growth, pointer-heavy layout, collision policy, and value storage in a
way that works against Wrela's memory model.

The primitive split is:

- `Table[T, N]` owns colocated row storage.
- `Index[K, N]` owns lookup metadata.
- An index maps keys to table rows; it does not own row data.

Initial tables and indexes use static const capacities. `Table[T, Rows]`
requires `T: Data`, and `Rows` is a `Const[U32]`.

Example:

```wrela
data Session {
    id: SessionId
    state: SessionState
    last_seen: Tick
}

class SessionStore {
    sessions: Table[Session, 4096]
    by_id: Index[SessionId, 8192]

    fn mark_active(mut self, id: SessionId, now: Tick) -> None {
        let row = by_id.require(id)

        sessions.state[row] = SessionState.Active
        sessions.last_seen[row] = now

        return None
    }
}
```

Different classes can choose different index strategies by owning different
index types. Index strategy should be a concrete type or type parameter, not a
runtime policy field:

- open-addressed index
- sorted index
- dense integer index
- bitmap-backed set/index
- direct table row id

Wrela should start without sugar over `Index + Table`. Users can compose their
own classes around tables and indexes. If a future pattern proves common, the
language can add indexed-table views later without changing the primitive memory
model.

Stable identity is not a property of `Table`. If stable identity is needed, use
an index, a durable handle, or a future `StableTable` type.

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
array-of-structs row in memory. Tables are also bounded storage: inserting past
declared capacity traps unless the table type explicitly advertises a
non-trapping policy.

Row tokens are scoped compiler capabilities. They cannot be stored, returned,
published, hidden inside data, or converted to pointers. Structural table
operations that can move rows are forbidden while row tokens for that table are
live.

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

Each mask carries table provenance and capacity. A mask produced from one table
cannot be accidentally applied to another table. Mask combination is allowed
only for compatible masks, and masked writes must prove non-overlapping mutable
access to the target columns.

This lets branches over many records become predicated operations instead of
scalar control-flow diamonds.

## Tables Plus Masks As The Main Vector Model

Tables and masks are the high-level vectorization surface. They let users write
bulk data operations in terms of records and conditions while giving the
compiler a columnar, predicated lowering target.

Example:

```wrela
class TimerTableOps {
    fn mark_ready(read self, timers: Table[TimerEntry], now: Tick) -> None {
        let expired = timers.deadline <= now

        timers.state[expired] = TimerState.Ready

        return None
    }
}
```

The compiler can lower this to scalar code, NEON-width batches, or a future SVE
strategy without changing the source-level semantics.

## Fixed Vector Types

Wrela should still include fixed vector value types for kernels where exact
lanes matter.

```wrela
class ByteVectorKernels {
    fn xor_block(read self, a: Vec[16, U8], b: Vec[16, U8]) -> Vec[16, U8] {
        return a ^ b
    }
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
vector programming model. Intrinsics are checked against the root-selected
target features. Assembly functions are allowed only inside `unique class`
authorities that own the relevant machine capability.

Example shape:

```wrela
use arch.aarch64.neon

class ChecksumKernels {
    fn checksum_step(read self, input: Vec[16, U8]) -> Vec[8, U16] {
        return neon.uaddlp(input)
    }
}
```

Assembly functions and intrinsics must be explicit about:

- Required CPU features, as provided by the root-selected target.
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

Wrela's initial external ABI target is AArch64 AAPCS64:

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

C interop should be minimal in v1. The first goal is appliance images and
hosted tests, not a broad foreign-function interface.

## Image Roots And Phases

Roots are composition declarations, not ordinary functions. They are the only
source of platform or host authority.

Root forms:

- `host image Name { phase run(host: unique MacOSHost) { ... } }`
- `image Name target QemuVirtPlatform { phase boot(platform: unique
  QemuVirtPlatform) { ... } }`

Root rules:

- Roots explicitly import every reachable module.
- Roots construct the authority graph.
- Roots narrow broad host/platform authority into smaller capabilities.
- Roots install fault policy.
- Roots select target platform, CPU features, and image/linker layout.
- Root phase declarations are not importable methods.
- Phase ordering is explicit.
- No module has initialization side effects outside the root graph.

Linker and image layout should be controlled by root-owned declarations:
named memory regions, sections, stacks, arenas, DMA regions, and boot entry
points.

## Test Execution Model

Wrela needs a permanent local test story without weakening its central model:
there are no ambient syscalls, no ambient linking, and no hidden dependency
graph. The same language should support fast tests on a development machine and
machine-level tests under QEMU while every capability still flows from a visible
root image.

If code runs, it is reachable from the root image. If code has power, the root
image granted it. Tests follow the same rule:

- Test suites are explicitly imported by the image root.
- Test suites are explicitly constructed by the image root.
- Test suites receive capabilities through constructor fields.
- Test blocks derive local fakes and fixtures from those suite capabilities.
- The runner executes already-constructed suites.
- The compiler may understand tests specially, but it does not smuggle tests,
  profiles, or host dependencies into the graph.

### Test Profiles

Wrela keeps a hosted profile permanently so unit and integration tests can run
quickly on the developer machine.

The major profiles are:

- `host image`: Builds a native executable for the development OS. It can
  access host-backed capabilities only through a unique host authority value.
- `image`: Builds a freestanding appliance or machine image. It can access
  hardware-backed capabilities only through platform authority discovered during
  boot.

The same generic test suite class can run in either profile when both roots can
provide concrete capability values satisfying the static interfaces it requires.

### Test Capability Interfaces

Tests should depend on behavior-shaped static interfaces, not
operating-system-shaped globals. Examples include:

- `Console`
- `Clock`
- `Memory`
- `BlockDevice`
- `ReadOnlyDirectory`
- `TempDirectory`
- `EntropySource`

Hosted roots can satisfy these with macOS-backed adapters. QEMU roots can
satisfy them with UARTs, architectural timers, claimed physical memory, virtio
devices, or other boot-visible hardware capabilities.

An interface name is used as a generic constraint, not as a hidden runtime
object. A suite that needs a console is shaped as `RingBufferTests<C: Console>`
with a field `console: C`.

Broad ambient interfaces such as a full `Filesystem` should be avoided unless
the test genuinely needs that authority. Prefer narrower capabilities such as a
read-only fixture directory or temporary scratch directory.

### Hosted Test Root

A hosted test root receives a unique host authority and narrows it into
explicit capabilities:

```wrela
use { RingBufferTests } from tests.ring_buffer
use { StorageTests } from tests.storage

host image HostTests {
    phase run(host: unique MacOSHost) {
        let console = host.stdout()
        let clock = host.monotonic_clock()
        let memory = host.test_arena(bytes: 64 * MiB)

        let runner = TestRunner(
            console = console,
            clock = clock,
        )

        runner.run([
            RingBufferTests(console = console),
            StorageTests(console = console, memory = memory),
        ])

        return None
    }
}
```

The host root is the only place where `MacOSHost` appears. Normal test suites
receive narrower concrete capabilities constrained by interfaces such as
`Console`, `Clock`, or `Memory`.

### QEMU Test Root

A QEMU test image wires the same suite classes to machine-backed capabilities:

```wrela
use { RingBufferTests } from tests.ring_buffer
use { StorageTests } from tests.storage

image QemuTests target QemuVirtPlatform {
    phase boot(platform: unique QemuVirtPlatform) {
        let console = UartConsole(platform.uart0.claim())
        let clock = GenericTimerClock(platform.timer.claim())
        let memory = BumpArena(platform.memory.claim_region(name = "test_heap"))

        let runner = TestRunner(
            console = console,
            clock = clock,
        )

        runner.run([
            RingBufferTests(console = console),
            StorageTests(console = console, memory = memory),
        ])

        return None
    }
}
```

The runner and suites are the same conceptual code. Only the root authority and
capability implementations change.

### Test Suite Classes

A test suite is a class with one or more `test` declarations. The class carries
its dependencies as fields. There is no required `name()` method and no explicit
registration method such as `runner.add`.

```wrela
module tests.ring_buffer

pub class RingBufferTests<C: Console> {
    console: C

    test "ring buffer correctly wraps" {
        let buffer = RingBuffer[U8](capacity = 4)

        buffer.push(1).unwrap()
        buffer.push(2).unwrap()
        buffer.push(3).unwrap()
        buffer.push(4).unwrap()

        let first_is_one = buffer.pop().unwrap() == 1
        assert value first_is_one

        buffer.push(5).unwrap()

        let second_is_two = buffer.pop().unwrap() == 2
        let third_is_three = buffer.pop().unwrap() == 3
        let fourth_is_four = buffer.pop().unwrap() == 4
        let fifth_is_five = buffer.pop().unwrap() == 5

        assert value second_is_two
        assert value third_is_three
        assert value fourth_is_four
        assert value fifth_is_five
    }
}
```

The compiler treats `test` as a declaration form. It can generate metadata and
dispatch for the runner, typecheck assertion behavior, and ensure only test
suites are passed to `TestRunner.run`.

### Test-Local Fixtures

Each test can declare local fakes or fixtures up front. These fixture
expressions are evaluated fresh for that test and can use the suite fields.

```wrela
module tests.storage

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

    fn read(mut self, index: U32) -> Result[Block, DiskError] {
        match index >= blocks {
            true => return Err(DiskError.OutOfRange)
            false => return Ok(storage[index])
        }
    }

    fn write(mut self, index: U32, block: Block) -> Result[None, DiskError] {
        match index >= blocks {
            true => return Err(DiskError.OutOfRange)
            false => {
                storage[index] = block
                return Ok(None)
            }
        }
    }
}

class FaultInjectingBlockDevice<D: BlockDevice> implements BlockDevice {
    inner: D
    fail_after_writes: U32
    writes: U32

    constructor(inner: D, fail_after_writes: U32) {
        return Self(
            inner = inner,
            fail_after_writes = fail_after_writes,
            writes = 0,
        )
    }

    fn read(mut self, index: U32) -> Result[Block, DiskError] {
        return inner.read(index = index)
    }

    fn write(mut self, index: U32, block: Block) -> Result[None, DiskError] {
        match writes >= fail_after_writes {
            true => return Err(DiskError.InjectedFailure)
            false => {
                writes += 1
                return inner.write(index, block)
            }
        }
    }
}

pub class StorageTests<C: Console, M: Memory> {
    console: C
    memory: M

    test "writes and reads one block"
        with disk = InMemoryBlockDevice(memory = memory, blocks = 1024)
    {
        let block = Block.filled(0xaa)

        disk.write(0, block).unwrap()

        let block_round_trips = disk.read(0).unwrap() == block
        assert value block_round_trips
    }

    test "propagates write failure"
        with disk = FaultInjectingBlockDevice(
            inner = InMemoryBlockDevice(memory = memory, blocks = 1024),
            fail_after_writes = 0,
        )
    {
        let block = Block.filled(0xaa)

        let propagates_failure = disk.write(0, block).is_error()
        assert value propagates_failure
    }
}
```

This keeps fake construction close to the tests that need it without bloating
the image root. Test-local fakes are module-private by default unless exported.

### Assertions

Wrela tests should not have a generic `assert expr` form. Assertions must state
which kind of equality or claim is being checked.

Initial assertion forms:

- `assert value claim`: checks a named value-equality or value-shaped boolean
  claim.
- `assert same claim`: checks a named identity claim: the same class instance,
  authority, storage identity, or identity-bearing capability.

Examples:

```wrela
test "services share parser identity" {
    let parser = HeaderParser(limits = limits)
    let service_a = SharedServiceA(parser = read parser)
    let service_b = SharedServiceB(parser = read parser)

    let share_parser = service_a.parser == service_b.parser
    assert same share_parser
}

test "parsed header matches expected value" {
    let actual = parser.parse(bytes).unwrap()
    let expected = Header(version = 1, length = 32)

    let header_matches = actual == expected
    assert value header_matches
}
```

Assertion failure is a test failure, not a trap. A trap inside a test is
reported separately as abnormal failure.

The assertion mode is part of typechecking:

- `assert value` is valid for scalar, enum, error, and `data` value claims.
- `assert same` is valid for classes, unique authorities, borrowed class
  references, and other identity-bearing capabilities.
- `assert same` is invalid for pure `data` values unless a future explicit
  handle type gives them identity.

### Runner Semantics

`TestRunner` is ordinary Wrela code plus a compiler-known call surface. It does
not discover source files, compile tests, or grant authority.

The runner receives:

- Its own concrete reporting capabilities constrained by interfaces such as
  `Console` and `Clock`.
- A list of already-constructed suite values.

`TestRunner` should follow the same static-interface rule as suites. A runner
with reporting dependencies is generic over concrete capability types rather
than storing interface-typed fields.

The runner performs:

- Iteration over compiler-known tests in each suite class.
- Per-test fixture setup.
- Test execution.
- Failure, assertion, trap, timeout, and summary reporting.

The runner does not perform:

- Source scanning.
- Runtime compilation.
- Automatic imports.
- Host or platform capability construction.

Runner summaries should be ordinary data:

```wrela
data TestSummary {
    passed: U32
    failed: U32
    trapped: U32
    timed_out: U32
}
```

Hosted process-exit mapping belongs in the hosted root, not inside suites.

### Test Typechecking And Dependency Graph

The compiler enforces the test model with these rules:

- `test` declarations are valid only in test-capable containers, initially
  classes.
- A class with at least one `test` declaration is a test suite.
- `TestRunner.run(suites)` accepts only values whose classes contain test
  metadata.
- A test body can access suite fields and its own `with` fixtures.
- A `with` fixture expression can use suite fields and earlier fixtures from the
  same test declaration.
- A `with` fixture is created fresh per test execution and destroyed after that
  test completes.
- Host authority roots such as `MacOSHost` and platform roots such as
  `QemuVirtPlatform` should not be passed into suites. Suites should receive
  narrowed capabilities.
- `assert value` accepts only value-shaped claims.
- `assert same` accepts only identity-bearing claims.

The image file is the test manifest. It explicitly imports suite classes and
constructs suite instances. There is no profile-based test discovery, no
`include profiles [host]`, and no compiler-injected manifest from arbitrary
source roots.

The compiler may generate internal metadata for suite classes that are already
reachable through imports, but it does not add new graph edges.

This design accepts a small amount of root ceremony to keep authority explicit.
The root lists suites, not every test case. Test-local fakes live beside the
tests. Different roots can instantiate the same suite class with different
capability implementations.

This gives the project a clean testing ladder:

- Pure tests use suites with no capabilities or only local data.
- Hosted integration tests use macOS-backed capabilities and test-local fakes.
- QEMU machine tests use hardware-backed capabilities.
- Real appliance tests can use the same suite pattern where practical.

## Classes And Interfaces

`class` is for behavior, capability, adapter, and suite composition. A class
must define at least one method or test declaration. Class fields are immutable
after construction: methods can mutate the state behind owned fields through
`mut self`, but cannot rebind fields.

```wrela
interface BlockDevice {
    fn read(mut self, index: U64, out: Buffer[U8]) -> Result[None, DiskError]
    fn write(mut self, index: U64, data: Buffer[U8]) -> Result[None, DiskError]
}

class VirtioBlockDevice implements BlockDevice {
    registers: unique VirtioBlockRegisters
    queue: unique VirtioQueue

    fn read(mut self, index: U64, out: Buffer[U8]) -> Result[None, DiskError] {
        return queue.submit_read(index = index, out = out)
    }
}
```

Classes are not the default representation for bulk records. If there are many
items of the same logical shape, prefer `data` plus `Table`.

Interfaces are static compile-time contracts. A method constrained by an
interface is statically resolved or monomorphized; it does not use an implicit
runtime vtable or hidden dynamic dispatch object.

## Vectorization Diagnostics

Vectorization should be reviewable at build time. The compiler should be able to
explain when a table/mask operation lowered cleanly and when it did not.

Possible diagnostic modes:

```wrela
class PacketClassifier {
    fn classify(read self, packets: Table[Packet]) -> None
        vectorize diagnose
    {
        let valid = packets.flags.has(PacketFlag.Valid)
        packets.flags[valid].set(PacketFlag.Checked)

        return None
    }

    fn classify_fast(read self, packets: Table[Packet]) -> None
        vectorize require
    {
        let large = packets.len > 1200
        packets.flags[large].set(PacketFlag.Jumbo)

        return None
    }
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
- Generic `if` as a separate core branching primitive.
- Generic `while` as a core loop primitive in the initial language.
- A traditional unbounded `for` loop as the default iteration form.
- Proving termination for intentional `loop` bodies.
- Exceptions or hidden stack unwinding.
- Ambient panic or process-exit behavior.
- Top-level free functions.
- Runtime vtables or implicit dynamic dispatch for interfaces.
- Runtime generic dictionaries or hidden type metadata for generics.
- Interface-typed fields as implicit existential objects.
- Interface-typed suite fields or hidden runtime capability objects.
- Developer-authored effect or permission clauses for privileged operations.
- Pointer arithmetic or raw memory dereference outside unique authority.
- Assembly functions outside unique authority.
- Implicit copying of class values.
- Mutable class-field rebinding after construction.
- Long-lived `mut` dependency fields in the initial language.
- Ambient heap allocation.
- General-purpose garbage collection or free.
- Built-in unbounded hash maps.
- Initial sugar over `Index + Table`.
- Broad C interop in v1.
- Wildcard imports in v1.
- Automatic source discovery for tests.
- Runtime compilation for tests.
- A hidden hosted standard library.
- Ambient filesystem, stdout, clock, allocator, or process access.
- A requirement that all tests be runnable under every profile.
- A generic `assert expr` form.

## Initial Language Shape

The first language nucleus should include:

- `module`, explicit `use` imports, `pub` visibility, and no module
  initialization side effects.
- static `interface` contracts with no implicit runtime vtables.
- compile-time generics over types and constants.
- capitalized generic constraints such as `Data`, `Copy`, `BlockDevice`, and
  `Const[U32]`.
- interface names usable as generic constraints, not hidden runtime field types.
- `class` for capability-carrying objects and adapters.
- class methods as the only ordinary callable function form.
- explicit class constructors that initialize every field exactly once.
- explicit receiver modes: `read self`, `mut self`, and `own self`.
- immutable class field bindings after construction.
- class values that move by default, with shared dependencies declared as
  `read`.
- `unique class` for authority-bearing owners with graph checks.
- authority-gated machine effects through unique capabilities.
- fixed-size primitive scalar types with explicit wrapping, saturating, and
  checked arithmetic.
- `match` as the core exhaustive scalar branching form.
- work-shaped loops: `repeat`, table-row `for`, `drain`, `reduce`, `scan`, and
  intentional `loop`.
- `return` as an explicit keyword in normal control flow.
- `Option[T]`, `Result[T, E]`, and closed `error` sums.
- `try`, `try else return`, and expanded `try else err { ... }`.
- `trap` as a `Never`-typed abnormal control-flow boundary.
- memory authority roots and bounded arenas.
- bounded memory views such as `Buffer[T]`, `ReadBuffer[T]`, `Bytes`, `Mmio[T]`,
  and `DmaBuffer[T]`.
- `with` frames for scoped scratch memory.
- `data` for logical records.
- `Table[T, N]` for static-capacity columnar bulk logical data.
- concrete index types for bounded lookup metadata over tables.
- table-provenance-aware `Mask` values for row selection and predication.
- `layout` for physical memory representation.
- `Vec[N, T]` for fixed vector kernels.
- `image` and `host image` roots, starting with QEMU `virt`, then Raspberry Pi
  5, then GCP cloud ARM VM.
- `test` declarations inside suite classes.
- `assert value` and `assert same` inside test blocks.

This gives Wrela a scalar authority model and a vector-friendly data model
without making either one masquerade as the other.

## Open Questions

The next design pass should still settle:

- Exact row-token type rules for table iteration without exposing row
  addresses.
- How table columns interact with ownership and borrowing.
- How masked writes report or forbid overlapping aliases.
- Exact syntax for custom reduction operators.
- Exact result type names for `scan` and whether scans can omit an empty body.
- Whether retry/poll loops need additional diagnostics beyond bounded `repeat`.
- Exact lifetime notation and diagnostics for `read` fields and borrowed class
  dependencies.
- Exact diagnostic shape for inferred effects and authority sources.
- Exact built-in generic constraint names beyond the initial capitalized set.
- Whether later backends share code between compatible generic instantiations.
- Whether `test` declarations are allowed only in classes or also modules.
- Exact diagnostic payloads for `assert value` and `assert same` failures.
- Exact hosted process exit mapping for `TestSummary`.
- How heterogeneous lists of generic test suite instances are represented
  without runtime interface objects.
- Exact arena type names for root, executor, driver, DMA, table, cache, and
  scratch memory.
- Exact `TrapCode` cases for OOM, capacity violations, bounds failures, and
  arithmetic traps.
- Whether `layout mmio` uses a distinct `Mmio[T]` field type or an enclosing
  layout rule.
- How `Vec[N, T]` values interact with ABI boundaries.
- How vectorization requirements are declared on methods, stages, or images.
- Whether Wrela should have first-class `pipeline` or `stage` declarations in
  addition to table/mask operations.
- Whether `match` is only statement-shaped or can also produce values in
  limited contexts.
- How image-installed fault policies interact with executor-local failures and
  whole-image halt/reboot behavior.
- Whether root phase declarations should share any syntax with methods or use a
  distinct entry syntax.
