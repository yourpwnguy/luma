# Architecture

## Goals

1. One effect clock shared by every device.
2. A device driver is one file plus one registry entry.
3. A slow device must not delay a fast one.
4. A crash, Ctrl-C, or normal exit must put the devices back into a safe state.
5. No firmware or bootloader commands.

Non-goals for v1: Windows, a tray app, audio-reactive effects, per-device
hardware effect synchronization, and daemon mode. The session and engine layers
are kept separate so a daemon can be added later without touching the drivers.

## Data flow

```text
main
  -> parse CLI
  -> load and validate TOML
  -> build Engine (one Effect instance)
  -> build and open drivers (capture restore snapshots)
  -> Session::start (one worker thread per device)
  -> drive loop:
       for each frame index n:
         target = start + n / fps
         sleep until target
         for each worker:
           colors = engine.render(n / fps, worker.layout, worker.stage)
           worker.publish(n, target, colors)
         poll worker events
         print stats
       stop on duration, Ctrl-C, or SIGTERM
  -> Session::shutdown:
       stop workers
       join workers and recover their device handles
       restore each device in startup order
```

The engine never sees a device type. It sees a `&[LedSpec]` and returns a
`Vec<Rgb>` of the same length. The driver maps that vector onto its own LED
order and wire format. The key consequence is that a new device with a
different LED count or geometry does not change the effect code.

## Modules

| Module | Responsibility |
| --- | --- |
| `main.rs` | Logger, signal registration, subcommand dispatch, exit code. |
| `cli.rs` | Clap types. No hardware or config logic. |
| `app.rs` | The run, probe, restore, and validation flows. |
| `drive.rs` | The shared frame loop, late-frame handling, and statistics. |
| `self_test.rs` | Visible color, key, and keymap test patterns. |
| `config.rs` | TOML schema, defaults, strict validation. |
| `engine.rs` | Applies an effect and brightness to one layout and its loop samples. |
| `effect/*` | Stateless color functions and the registry. |
| `geometry.rs` | Normalized `Point`, `LedSpec`, and `LedSample` (local position plus shared loop position). |
| `stage.rs` | Maps each device layout into its slice of the shared 0..1 loop. |
| `color.rs` | RGB, HSV, hex, scaling. |
| `sync.rs` | Mailboxes, worker threads, deadlines, latency EWMA, reconnect, shutdown. |
| `device/mod.rs` | `Device` trait, frame length check, hidraw open helper. |
| `device/sysfs.rs` | VID/PID, interface, and report-ID based discovery. |
| `device/registry.rs` | Driver factory, dry-run mocks, stale-free discovery. |
| `device/g102.rs` | G203L direct protocol. |
| `device/k617.rs` | Sinodragon report 8 frames and the 136-byte config snapshot. |
| `device/k617_layout.rs` | The 61-key physical layout and LED slot table. |
| `device/asus_tuf.rs` | `asus-wmi` sysfs ABI. |

## Shared device loop

Per-device effects only need the local LED position. Cross-device flow effects
need a common order, so `stage.rs` maps each device onto a 0..1 loop. The
default spans are:

```text
keyboard 0.00..0.45  ->  mouse 0.45..0.70  ->  laptop 0.70..0.85  ->  gap 0.85..1.00
```

A multi-LED device maps its local `x` into its span. A one-LED device uses the
middle of its span. The mapping is computed once in `Session::start` and stored
next to the worker's layout, so the frame loop does no lookup or geometry work.
The engine passes both values to the effect as `LedSample { local, loop_s }`.
The per-device effects read `local`; `flow`, `orbit`, `chase`, `pulse`, and
`pingpong` read `loop_s`.

`StageConfig` is optional. An explicit `[[stage.devices]]` entry overrides the
kind default, which lets the user reverse the order or change how long a device
stays active without touching code. The 0.85..1.00 gap is intentionally dark:
it gives the loop a visible restart instead of an invisible wrap.

The loop is a timing and ordering model, not a 2D simulation of the desk. That
is deliberate. It gives a predictable, readable flow on a 61-key keyboard, a
three-zone mouse, and a one-zone laptop without pretending the laptop can show
a spatial gradient.

## Timing model

Let `t0` be a monotonic `Instant` captured before the first frame. Frame `n`
has two values:

* Effect time: `n / fps` seconds.
* Display target: `t0 + n / fps`.

The effect time is based on the frame index, not the wall clock. A delayed or
skipped frame therefore does not change the phase of the wave. If the process
falls more than two frame periods behind, it jumps the frame index to the
current position and logs one warning.

Each worker receives the same target. It estimates its write duration `L` with
an exponential moving average and starts the write at `target - L/2`, so the
middle of the USB control transfer or sysfs write lands on the target. The
K617 measured around 6.8 ms per 382-byte feature report, the G102 around 3.3 ms
for two 20-byte writes, and the ASUS around 0 ms for a sysfs write on this
machine. The compensation matters most for the K617.

The mailbox has one slot. If the engine produces a newer frame before a worker
writes the old one, the worker overwrites the old frame. This is intentional:
showing the newest state is better than replaying a backlog. A frame older than
100 ms is discarded. `max_fps` drops frames at the worker, so the ASUS can run
at 15 fps while the engine runs at 30 fps.

## Ownership and concurrency

* The main thread owns the `Engine` and the `Session`.
* Each worker owns exactly one `Box<dyn Device>`.
* `Session` owns the worker join handles and the restore responsibility.
* A worker that loses its device closes it, attempts six reconnects with
  exponential backoff, and then returns a fatal event.
* The worker returns its device through the join result even if the loop
  panicked. The main thread performs restore after every worker has stopped.

This is why restore is not implemented in a `Drop` on the driver. A driver drop
cannot report an error or wait for the other devices. `Session` is the single
place that owns shutdown ordering.

## Restore lifecycle

`open()` captures whatever the protocol allows:

| Driver | Snapshot | Restore |
| --- | --- | --- |
| K617 | 136-byte config block, read with report `0x05`/`0x06` | Write the exact block with report `0x06`. |
| G102 | Nothing, the direct protocol has no readback | Apply the configured default color or off. |
| ASUS | Current brightness level | Apply the configured default color and the captured brightness. |

The snapshot is captured once. A reconnect does not overwrite it. If a K617
snapshot cannot be read, the driver refuses to run by default; set
`options.allow_missing_snapshot = true` to accept a solid fallback restore.

`Session::shutdown` runs on normal exit, on SIGINT/SIGTERM through the stop
flag, and from `Drop` during unwinding. SIGKILL cannot be handled and will
leave the last frame on the devices.

## Error policy

`on_error = "continue"` is the default. A failed device is retried by its own
worker while the other devices keep rendering. `on_error = "stop"` turns the
first permanent failure into a run failure. A worker panic always fails the
run, because continuing with an unknown device state is worse than stopping.

## Testing

The unit tests cover color math, effect determinism, packet builders, slot
uniqueness and anchors, config validation, the ASUS sysfs file behavior, the
mock device, and the mailbox. `--dry-run` exercises the full engine, worker
threads, mailboxes, stats, and shutdown with the real layouts but no hardware.
`scripts/live-test.sh` is the visible hardware test.

The protocol tests deliberately assert exact byte layouts. A future refactor
that reorders a packet header should fail a test, not silently produce a
different lighting command.
