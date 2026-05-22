# Language Primitives and Data Layout Design

Date: 2026-05-22

## Purpose

Wrela is an AArch64-only systems language for building complete appliance
images. The language should make machine authority, memory ownership, data
layout, and hot data paths visible to the compiler without turning all code
into assembly or compiler folklore.

This design captures the initial shape of the language nucleus:

- Scalar control and authority code remains explicit and readable.
- Modules do not allow top-level `fn` declarations.
- Callable functions are methods under classes.
- Interfaces are static compile-time contracts, not runtime vtables.
- Class fields are immutable after construction.
- Class values move by default; shared dependencies must be `read`.
- Generics are compile-time only and use capitalized constraints.
- Interface names are static constraints, not hidden runtime field types.
- Scalar branching uses exhaustive `match`, not a generic `if`.
- Methods and phase blocks use explicit `return`.
- Recoverable errors are typed values, not exceptions.
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
- `with` fixtures for test-local fake construction.
- Explicit root images that import and construct suite classes.

This document focuses on the broader language shape underneath that test model.

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

Tables should initially require `T: Data`, not arbitrary classes. This
preserves the columnar storage and vectorization model. Classes can own tables
and indexes, but a table should not become a bag of hidden object identities.

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

image QemuTests {
    phase boot(platform: unique QemuVirt) {
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
image PacketAppliance {
    phase boot(platform: unique QemuVirt) {
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
index types:

- open-addressed index
- sorted index
- dense integer index
- bitmap-backed set/index
- direct table row id

Wrela should start without sugar over `Index + Table`. Users can compose their
own classes around tables and indexes. If a future pattern proves common, the
language can add indexed-table views later without changing the primitive memory
model.

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
vector programming model.

Example shape:

```wrela
use arch.aarch64.neon

class ChecksumKernels {
    fn checksum_step(read self, input: Vec[16, U8]) -> Vec[8, U16]
        requires cpu.adv_simd
    {
        return neon.uaddlp(input)
    }
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
- Exceptions or hidden stack unwinding.
- Ambient panic or process-exit behavior.
- Top-level free functions.
- Runtime vtables or implicit dynamic dispatch for interfaces.
- Runtime generic dictionaries or hidden type metadata for generics.
- Interface-typed fields as implicit existential objects.
- Implicit copying of class values.
- Mutable class-field rebinding after construction.
- Long-lived `mut` dependency fields in the initial language.
- Ambient heap allocation.
- General-purpose garbage collection or free.
- Built-in unbounded hash maps.
- Initial sugar over `Index + Table`.

## Initial Language Shape

The first language nucleus should include:

- `module` and explicit `use` imports.
- static `interface` contracts with no implicit runtime vtables.
- compile-time generics over types and constants.
- capitalized generic constraints such as `Data`, `Copy`, `BlockDevice`, and
  `Const[U32]`.
- interface names usable as generic constraints, not hidden runtime field types.
- `class` for capability-carrying objects and adapters.
- class methods as the only ordinary callable function form.
- explicit receiver modes: `read self`, `mut self`, and `own self`.
- immutable class field bindings after construction.
- class values that move by default, with shared dependencies declared as
  `read`.
- `unique class` for authority-bearing owners with graph checks.
- `match` as the core exhaustive scalar branching form.
- `return` as an explicit keyword in normal control flow.
- `Option[T]`, `Result[T, E]`, and closed `error` sums.
- `try`, `try else return`, and expanded `try else err { ... }`.
- `trap` as a `Never`-typed abnormal control-flow boundary.
- memory authority roots and bounded arenas.
- `with` frames for scoped scratch memory.
- `data` for logical records.
- `Table[T]` for columnar bulk logical data.
- `Index[K, N]` for bounded lookup metadata over tables.
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
- Exact `Index` strategy syntax and whether strategy is a type, constructor, or
  policy field.
- Whether `Mask` is parameterized by capacity, table identity, or lane count.
- How table row iteration works without exposing row addresses.
- How table columns interact with ownership and borrowing.
- How masked writes report or forbid overlapping aliases.
- Exact lifetime notation and diagnostics for `read` fields and borrowed class
  dependencies.
- Whether any class field mode beyond owned and `read` should exist after v1.
- Exact built-in generic constraint names beyond the initial capitalized set.
- Whether generic type arguments are always inferred from constructors or can be
  explicitly supplied at construction sites.
- Whether later backends share code between compatible generic instantiations.
- Exact arena type names for root, executor, driver, DMA, table, cache, and
  scratch memory.
- Exact trap report payload for OOM and capacity violations.
- Whether `layout mmio` uses a distinct `Mmio[T]` field type or an enclosing
  layout rule.
- How `Vec[N, T]` values interact with ABI boundaries.
- How vectorization requirements are declared on methods, stages, or images.
- Whether Wrela should have first-class `pipeline` or `stage` declarations in
  addition to table/mask operations.
- Whether `match` is only statement-shaped or can also produce values in
  limited contexts.
- Exact syntax for trap reports and source-location payloads.
- How image-installed fault policies interact with executor-local failures and
  whole-image halt/reboot behavior.
- Whether root phase declarations should share any syntax with methods or use a
  distinct entry syntax.
