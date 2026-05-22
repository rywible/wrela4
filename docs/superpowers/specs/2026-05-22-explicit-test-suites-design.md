# Explicit Test Suites Design

Date: 2026-05-22

## Purpose

Wrela needs a permanent local test story without weakening its central model:
there are no ambient syscalls, no ambient linking, and no hidden dependency
graph. The same language should support fast tests on a development machine and
machine-level tests under QEMU, while every capability still flows from a
visible root image.

This design defines hosted and QEMU test execution as ordinary image
composition. The host target is a first-class development profile, not a
semantic loophole.

## Core Principle

If code runs, it is reachable from the root image. If code has power, the root
image granted it.

Tests follow the same rule:

- Test suites are explicitly imported by the image root.
- Test suites are explicitly constructed by the image root.
- Test suites receive capabilities through constructor fields.
- Test blocks derive local fakes and fixtures from those suite capabilities.
- The runner executes already-constructed suites.
- The compiler may understand tests specially, but it does not smuggle tests,
  profiles, or host dependencies into the graph.

## Profiles

Wrela keeps a hosted profile forever so unit and integration tests can run
quickly on the developer machine.

The major profiles are:

- `host image`: Builds a native executable for the development OS. It can access
  host-backed capabilities only through a unique host authority value.
- `image`: Builds a freestanding appliance or machine image. It can access
  hardware-backed capabilities only through platform authority discovered during
  boot.

The same test suite class can run in either profile when both roots can provide
implementations of the interfaces it requires.

## Capability Interfaces

Tests should depend on behavior-shaped interfaces, not operating-system-shaped
globals. Examples include:

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

Broad ambient interfaces such as a full `Filesystem` should be avoided unless
the test genuinely needs that authority. Prefer narrower capabilities such as a
read-only fixture directory or temporary scratch directory.

## Hosted Root Shape

A hosted test root receives a unique host authority and narrows it into explicit
capabilities:

```wrela
use { RingBufferTests } from tests.ring_buffer
use { StorageTests } from tests.storage

host image HostTests {
    fn run(host: unique MacOSHost) -> None {
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
receive narrower capabilities such as `Console`, `Clock`, or `Memory`.

## QEMU Root Shape

A QEMU test image wires the same suite classes to machine-backed capabilities:

```wrela
use { RingBufferTests } from tests.ring_buffer
use { StorageTests } from tests.storage

image QemuTests {
    phase boot(platform: unique QemuVirt) {
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

## Test Suite Classes

A test suite is a class with one or more `test` declarations. The class carries
its dependencies as fields. There is no required `name()` method and no explicit
registration method such as `runner.add`.

```wrela
module tests.ring_buffer

pub class RingBufferTests {
    console: Console

    test "ring buffer correctly wraps" {
        let buffer = RingBuffer[U8](capacity = 4)

        buffer.push(1).unwrap()
        buffer.push(2).unwrap()
        buffer.push(3).unwrap()
        buffer.push(4).unwrap()

        assert buffer.pop().unwrap() == 1
        buffer.push(5).unwrap()

        assert buffer.pop().unwrap() == 2
        assert buffer.pop().unwrap() == 3
        assert buffer.pop().unwrap() == 4
        assert buffer.pop().unwrap() == 5
    }
}
```

The compiler treats `test` as a declaration form. It can generate metadata and
dispatch for the runner, typecheck assertion behavior, and ensure only test
suites are passed to `TestRunner.run`.

## Test-Local Fixtures

Each test can declare local fakes or fixtures up front. These fixture
expressions are evaluated fresh for that test and can use the suite fields.

```wrela
module tests.storage

class InMemoryBlockDevice implements BlockDevice {
    memory: Memory
    blocks: UInt
    storage: BlockArray

    constructor(memory: Memory, blocks: UInt) {
        return Self(
            memory = memory,
            blocks = blocks,
            storage = BlockArray.allocate(memory = memory, blocks = blocks),
        )
    }

    fn read(index: UInt) -> Result[Block, DiskError] {
        match index >= blocks {
            true => return Err(DiskError.OutOfRange)
            false => return Ok(storage[index])
        }
    }

    fn write(index: UInt, block: Block) -> Result[None, DiskError] {
        match index >= blocks {
            true => return Err(DiskError.OutOfRange)
            false => {
                storage[index] = block
                return Ok(None)
            }
        }
    }
}

class FaultInjectingBlockDevice implements BlockDevice {
    inner: BlockDevice
    fail_after_writes: UInt
    writes: UInt = 0

    fn write(index: UInt, block: Block) -> Result[None, DiskError] {
        match writes >= fail_after_writes {
            true => return Err(DiskError.InjectedFailure)
            false => {
                writes += 1
                return inner.write(index, block)
            }
        }
    }
}

pub class StorageTests {
    console: Console
    memory: Memory

    test "writes and reads one block"
        with disk = InMemoryBlockDevice(memory = memory, blocks = 1024)
    {
        let block = Block.filled(0xaa)

        disk.write(0, block).unwrap()

        assert disk.read(0).unwrap() == block
    }

    test "propagates write failure"
        with disk = FaultInjectingBlockDevice(
            inner = InMemoryBlockDevice(memory = memory, blocks = 1024),
            fail_after_writes = 0,
        )
    {
        let block = Block.filled(0xaa)

        assert disk.write(0, block).is_error()
    }
}
```

This keeps fake construction close to the tests that need it without bloating
the image root. Test-local fakes are module-private by default unless exported.

## Runner Semantics

`TestRunner` is ordinary Wrela code plus a compiler-known call surface. It does
not discover source files, compile tests, or grant authority.

The runner receives:

- Its own reporting capabilities, such as `Console` and `Clock`.
- A list of already-constructed suite values.

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

## Typechecking Rules

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
- Host authority roots such as `MacOSHost` and platform roots such as `QemuVirt`
  should not be passed into suites. Suites should receive narrowed
  capabilities.

## Dependency Graph

The image file is the test manifest. It explicitly imports suite classes and
constructs suite instances.

There is no profile-based test discovery. There is no `include profiles [host]`.
There is no compiler-injected manifest from arbitrary source roots.

The compiler may generate internal metadata for suite classes that are already
reachable through imports, but it does not add new graph edges.

## Ergonomics

This design accepts a small amount of root ceremony to keep authority explicit.
The root lists suites, not every test case. Test-local fakes live beside the
tests. Different roots can instantiate the same suite class with different
capability implementations.

This gives the project a clean testing ladder:

- Pure tests use suites with no capabilities or only local data.
- Hosted integration tests use macOS-backed capabilities and test-local fakes.
- QEMU machine tests use hardware-backed capabilities.
- Real appliance tests can use the same suite pattern where practical.

## Non-Goals

This design does not include:

- Automatic source discovery for tests.
- Runtime compilation.
- A hidden hosted standard library.
- Ambient filesystem, stdout, clock, allocator, or process access.
- A requirement that all tests be runnable under every profile.

## Open Language Questions

The following language primitives still need to be designed:

- Exact syntax and semantics for `interface`.
- Exact syntax and semantics for `class` fields and constructors.
- Whether `test` declarations are allowed only in classes or also modules.
- How `with` fixture lifetimes interact with ownership and borrowing.
- How assertions are represented in the type system.
- How test failures differ from traps and fault-policy reports.
- How runner result reporting is modeled without hidden process exit behavior.
- How arrays of heterogeneous test suites are represented.

These questions should be handled while designing the language nucleus. The
initial language/data-layout direction is captured in
`docs/superpowers/specs/2026-05-22-language-primitives-and-data-layout-design.md`.
