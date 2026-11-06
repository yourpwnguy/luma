# luma

One lighting clock for three devices that normally have nothing to do with
each other:

| Config id | Device | USB / interface |
| --- | --- | --- |
| `keyboard` | Redragon K617 Fizz | `258a:0049`, hidraw interface 1, vendor report `0x08` |
| `mouse` | Logitech G102 LIGHTSYNC (G203 LIGHTSYNC) | `046d:c092`, hidraw interface 1, report `0x11` |
| `laptop` | ASUS TUF Gaming A15 FA506 keyboard backlight | `asus-wmi` LED sysfs, one zone |

The tool renders one effect at one shared clock, maps it to each device's
geometry, and pushes frames on absolute deadlines. It talks to the devices
directly over hidraw and sysfs. It does not need G HUB, the Redragon Windows
software, asusctl, OpenRGB, or a kernel driver detach.

Linux is the supported platform for v1. The hardware in this repository was
validated on Arch Linux with kernel 7.3.1 on an ASUS TUF A15 FA506IHRB.

## What is verified

This is the short version. [`docs/PROTOCOLS.md`](docs/PROTOCOLS.md) has the
packet layouts, source links, and the exact boundary between "we saw this on
the wire" and "upstream says this should work".

| Area | Status |
| --- | --- |
| K617 per-key frame report `0x08`, 382 bytes | Verified live. 59 frames in 2 s, 6.8 ms average write latency, no errors. |
| K617 config snapshot and exit restore | Verified live. Snapshot decoded as mode 17 with the original wave preset, and the wave came back after a run. |
| K617 red/green color order | Verified visually. Red and green were not swapped. |
| K617 slot map for Esc, Tab, Caps, LShift, LCtrl, Space, J, Enter | Verified upstream on K617 hardware by the fizz-rgb project. |
| K617 Menu and RCtrl slots | Upstream best guess. Use `self-test --keys Menu,RCtrl` to confirm on your unit. |
| G102 three-zone direct packets | Verified live and visually. All three zones showed red and green. |
| G102 restore | Defaults to configured white because the protocol has no readback. |
| ASUS single-zone color and brightness | Verified live and visually. The ACPI write changed the keyboard color. |
| ASUS exit restore | Restores a configured default color and the captured brightness. The mode is write-only, so the previous mode cannot be read. |
| Synchronization | 30 fps engine, absolute deadlines, per-device latency compensation. K617 and G102 measured near 30 fps; ASUS runs at 15 fps. |

The one thing still pending on this exact unit is the physical confirmation of
the Menu and RCtrl slot positions. Everything else in the table was exercised
against the attached hardware.

## Why Rust

Rust is the better fit here than Python or C++:

* The job is a 30 Hz loop with per-device worker threads, absolute deadlines,
  and a bounded mailbox. Rust makes the ownership of the device handles and the
  frame buffers explicit, and the release binary is a single file with no
  virtualenv or runtime dependency.
* The type system is useful for the protocol work. Packet lengths, color
  buffers, and snapshot sizes are encoded in types and checked by tests.
* `hidapi` has a stable Rust binding and a Linux hidraw backend. The code only
  needs the kernel's hidraw and sysfs interfaces.
* Cross-compilation and static linking are straightforward if the project ever
  grows a Windows backend.

Alternatives considered:

* **Python**: fastest to prototype and the K617/OpenRGB ecosystems have Python
  examples. But packaging a timed 30 Hz daemon, keeping the worker threads from
  fighting the GIL, and shipping a predictable binary are all more work. The
  performance is sufficient; the operational story is not as good.
* **C++**: this is what OpenRGB uses and the ecosystem is mature. It is also a
  lot of build system and memory-safety surface for a small tool.
* **Go**: good concurrency, but the HID ecosystem is thinner and the low-level
  ioctl work would end up in cgo anyway.

## What was reused versus what was written

The protocol research did not start from zero. These projects were used as
evidence, not linked as dependencies:

| Source | Reused |
| --- | --- |
| [OpenRGB](https://github.com/CalcProgrammer1/OpenRGB) | G203L direct packets and the ASUS TUF sysfs ABI. |
| [fizz-rgb](https://github.com/MrSchrodingers/fizz-rgb) | K617 report `0x08` frame layout and the physical LED slot map. |
| [redragon-fizz-k617-rgb](https://github.com/MarceloAntonio/redragon-fizz-k617-rgb) | K617 136-byte config read/write. |
| [Linux asus-wmi](https://github.com/torvalds/linux/blob/master/drivers/platform/x86/asus-wmi.c) | The authoritative `kbd_rgb_mode` and `kbd_rgb_state` ABI. |
| [redragon-ks82b-rgb](https://github.com/kn4oqw-clint/redragon-ks82b-rgb) | An independent 382-byte Sinowealth LED report implementation. |

Written for this project:

* The unified effect engine, spatial mapping, and absolute-deadline scheduler.
* The per-device worker, latest-frame mailbox, latency compensation, reconnect,
  and restore lifecycle.
* The K617, G102, and ASUS drivers as self-contained Rust modules.
* Discovery by VID/PID, interface number, and report ID.
* The TOML schema, CLI, dry-run mocks, self-tests, and live-test script.

Why not just use OpenRGB? It is an excellent device manager and the protocol
reference for two of the three devices, but it does not provide one shared
effect clock across devices, K617 streaming, or the restore semantics this tool
needs. Why not asusctl? It is ROG-focused and does not drive the other two
devices. Why not QMK/VIA? The K617 is not a QMK keyboard; the QMK port is a
hardware modification. The Redragon Windows software is not used at all.

One safety note: the ks82b project reports that OpenRGB's Sinowealth driver was
disabled because of bricking reports tied to its firmware-flash path. luma
does not use that path regardless. It sends only the LED frame and lighting
config reports listed in [`docs/PROTOCOLS.md`](docs/PROTOCOLS.md).

## Architecture in one picture

```text
                 +-------------------------------+
                 |  Engine (one monotonic clock)  |
                 |  effect.color_at(t, position)  |
                 +---------------+---------------+
                                 |
              one color vector per device per frame
                                 |
        +------------------------+------------------------+
        |                        |                        |
  +-----v------+          +------v-----+           +------v------+
  | K617 worker|          | G102 worker|           | ASUS worker |
  | report 0x08|          | report 0x11|           | sysfs write |
  | 30 fps     |          | 30 fps     |           | 15 fps      |
  +-----+------+          +------+-----+           +------+------+
        |                        |                        |
  /dev/hidraw3             /dev/hidraw1        /sys/class/leds/...
```

Each worker owns its device handle. The engine publishes a frame into a
single-slot mailbox. Publishing overwrites an unwritten frame, so a slow device
skips ahead instead of building a latency backlog. Every frame carries an
absolute `Instant` target, and each worker learns its own write duration and
starts the transfer half a latency early. The design is described in
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Project layout

```text
Cargo.toml
config/luma.toml          sample config
src/
  main.rs                    CLI entry point and signal handlers
  cli.rs                     clap definitions
  app.rs                     run, probe, restore, config validation
  drive.rs                   the shared frame loop and latency statistics
  self_test.rs               visible test patterns
  config.rs                  TOML schema and validation
  engine.rs                  one effect clock, per-device rendering
  effect/                    one file per effect and the registry
  geometry.rs                normalized LED positions
  color.rs                   RGB, HSV, hex parsing
  sync.rs                    worker threads, mailboxes, deadline scheduler
  device/
    mod.rs                   Device trait and hidraw helper
    registry.rs              driver factory and discovery
    sysfs.rs                 hidraw/sysfs discovery
    g102.rs                  Logitech G102 driver
    k617.rs                  Redragon K617 protocol driver
    k617_layout.rs           K617 physical layout and LED slot table
    asus_tuf.rs              ASUS TUF sysfs driver
    mock.rs                  in-memory driver for --dry-run and tests
scripts/
  install-permissions.sh     one-time udev rule install
  live-test.sh               visible hardware validation
udev/70-luma.rules        least-privilege device permissions
docs/
  ARCHITECTURE.md
  PROTOCOLS.md
```

## Install

### Dependencies

On Arch Linux:

```bash
sudo pacman -S --needed base-devel rustup
rustup default stable
cargo build --release
```

`base-devel` is needed because the bundled hidapi backend is C. There is no
Python, Node, or Qt dependency, and no kernel module to build.

The examples below use `luma` as the command name. Either run
`./target/release/luma` directly or install it into your Cargo bin directory:

```bash
cargo install --path . --locked
```

### Permissions

The USB devices expose interface 0 (normal input) and interface 1 (vendor
control). The udev rule grants the active seat and the `input` group access to
interface 1 only. The ASUS attributes are write-only sysfs files, so the rule
makes exactly those two files writable.

```bash
sudo ./scripts/install-permissions.sh
```

The installer also removes the old `70-rgbsync.rules` from the pre-rename
version, so rerun it once after switching to luma.

Replug the keyboard and mouse if the hidraw nodes do not update immediately.
Verify with:

```bash
luma list-devices
```

If you write your own udev rule, do not combine
`ATTRS{bInterfaceNumber}=="01"` with `ATTRS{idVendor}`/`ATTRS{idProduct}` in one
rule. The vendor/product attributes live on the USB device parent, while
`bInterfaceNumber` lives on the interface parent, and udev requires all `ATTRS`
matches in one rule to resolve on the same parent. Use the
`ENV{ID_USB_INTERFACE_NUM}=="01"` property as this repository does.

Expected on this machine:

```text
g102      present   /dev/hidraw1 (interface 1, report 0x11)
k617      present   /dev/hidraw3 (interface 1, report 0x08)
asus_tuf  present   /sys/class/leds/asus::kbd_backlight (single zone, max brightness 3)
```

The ASUS path needs a kernel with `asus-wmi` TUF RGB support (6.2 or newer) and
the `asus_nb_wmi` module. Check:

```bash
ls /sys/class/leds/asus::kbd_backlight/kbd_rgb_mode
modinfo asus_nb_wmi
```

### Safety

* No firmware flashing, bootloader entry, or EEPROM writes. The K617 driver only
  sends report `0x08` frames and reports `0x05`/`0x06` for the 136-byte lighting
  config. It never sends the Sinowealth ISP sequence.
* The G102 driver only sends the three-zone direct-mode and apply packets.
* The ASUS driver writes `kbd_rgb_mode` with `save=0`, which the kernel maps to
  the temporary command `0xb3`, not the BIOS-save command `0xb4`. It never
  touches `kbd_rgb_state`, the boot/sleep/shutdown lighting state.
* Restore runs on normal exit, Ctrl-C, SIGTERM, and unwinding panics. SIGKILL
  and power loss cannot be intercepted.
* `--no-restore` is explicit and leaves the last frame visible.

## Usage

```bash
# List devices and the resolved nodes
./target/release/luma list-devices

# Read-only probe, including the G102 HID++ feature table
./target/release/luma probe

# Run the default config until Ctrl-C
./target/release/luma run

# The requested example
./target/release/luma run --effect wave --speed 2

# Stop after 10 seconds
./target/release/luma run --effect rainbow --duration 10

# Only the keyboard and mouse
./target/release/luma run --effect cycle --devices keyboard,mouse

# Exercise the whole pipeline without touching hardware
./target/release/luma run --dry-run --effect wave --duration 3

# Visible tests
./target/release/luma self-test
./target/release/luma self-test --keys Escape,Menu,RCtrl
./target/release/luma self-test --keymap

# Validate the config without opening anything
./target/release/luma validate-config

# Recovery: open the devices and apply their configured restore state now
./target/release/luma restore
```

`--speed` is a multiplier, not a frequency. At speed 1.0, breathing has a two
second period, wave moves one spatial cycle per two seconds, and cycle takes ten
seconds for a full hue rotation.

### Config

The default config is `config/luma.toml` when run from the repository root,
then `$XDG_CONFIG_HOME/luma/luma.toml`, then
`$HOME/.config/luma/luma.toml`. Use `--config FILE` to override.

```toml
[general]
fps = 30
duration_secs = 30
on_error = "continue"       # or "stop"
stats_interval_secs = 5.0

[effect]
name = "wave"
color = "#00aaff"
speed = 1.0
brightness = 0.8
wave_cycles = 2.0

[[devices]]
id = "keyboard"
type = "k617"
enabled = true
required = true
max_fps = 30

[[devices]]
id = "mouse"
type = "g102"
max_fps = 30
[devices.restore]
policy = "default"
color = "#ffffff"
brightness = 0.30

[[devices]]
id = "laptop"
type = "asus_tuf"
max_fps = 15
[devices.options]
hardware_brightness = 3
[devices.restore]
policy = "default"
color = "#ffffff"
hardware_brightness = 2
```

Restore policies:

| Policy | Behavior |
| --- | --- |
| `previous` | K617 writes the captured 136-byte config back. G102 and ASUS fall back to a configured default because their protocols cannot read the current color. |
| `default` | Apply the configured color and brightness. |
| `off` | Set the LEDs black. |
| `leave` | Do not touch the device on exit. |

The sample config restores the K617 to its previous wave preset and sets the
mouse and laptop to dim white. That means the devices intentionally stop
matching after the run ends. If you want a uniform idle look, set the K617
restore policy to `default` with the same color and brightness as the others.

`required = false` makes a missing device a warning instead of a fatal error.
`max_fps` is a ceiling; the ASUS EC defaults to 15 fps, which is the largest
clean divisor of a 30 fps engine.

## Effects

| Name | What it does |
| --- | --- |
| `static` | Solid configured color. |
| `off` | All LEDs black. |
| `breathing` | Base color with a two-second breathing envelope at speed 1. |
| `rainbow` | Spatial rainbow, hue varies along x and drifts over time. |
| `wave` | Moving brightness bands of the base color. |
| `cycle` | One global hue cycle. Best for checking the single-zone laptop against the others. |

The engine samples every LED's normalized position from 0 to 1. The K617 has
61 positions in a 6-row layout, the G102 has left/center/right at 0/0.5/1, and
the ASUS has one LED at the center. A single-zone device therefore follows the
center of the same wave instead of pretending to have per-key resolution.

## Adding an effect

1. Add `src/effect/my_effect.rs`.
2. Implement `Effect`:

   ```rust
   impl Effect for MyEffect {
       fn name(&self) -> &'static str { "my_effect" }
       fn color_at(&self, ctx: &EffectContext, position: Point) -> Rgb {
           // Return full-intensity color. The engine applies brightness.
       }
   }
   ```

3. Register it in `src/effect/mod.rs` in `build()` and add a row to `NAMES`.
4. Add a deterministic unit test. Time and position are the only inputs.
5. Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and
   `cargo test`.
6. Try it with `luma run --dry-run --effect my_effect --duration 2`, then on
   hardware.

Effects are stateless by design so all devices share one phase. If you need
state (fire, sparkle, audio), extend the `Effect` trait with a shared update
step rather than keeping per-device state.

## Adding a device

1. Add `src/device/my_device.rs`.
2. Implement `Device`: `id`, `kind`, `model`, `layout`, `open`, `send`,
   `restore`, `close`, `is_open`.
3. Build `layout()` with one `LedSpec` per addressable LED and normalized
   positions. Return colors from `send` in that same order.
4. Capture a restore snapshot in `open()` if the protocol supports reading it.
   Do not overwrite the snapshot on reconnect.
5. Add a match arm in `src/device/registry.rs::build()` and a discovery entry in
   `discover()`.
6. Add a mock layout in `registry::mock_for()` so `--dry-run` works.
7. Add packet unit tests and, if possible, a read-only `probe` path.
8. Add the device to `config/luma.toml` and the udev rule if it needs
   permissions.

The engine never needs to change. If the new device has fewer LEDs, it simply
gets sampled fewer times.

## Synchronization and latency

All devices get the same `seq` and `target: Instant`. The engine's clock is
based on the frame index, not wall time, so a delayed frame does not change the
effect phase. Each worker:

1. Takes the latest frame.
2. Drops it if it is more than 100 ms late.
3. Sleeps until `target - measured_latency / 2`.
4. Writes the frame and updates an exponential moving average of the write
   duration.

The stats line printed every five seconds looks like this:

```text
sync stats: keyboard: 59f 6.8ms late=0 drop=0 err=0 | mouse: 59f 3.2ms late=0 drop=0 err=0 | laptop: 30f 0.0ms late=0 drop=29 err=0
```

`late` counts writes that started more than 2 ms after their target. `drop`
counts frames intentionally skipped to keep a slow device from lagging behind.
On a loss, a worker closes the device, retries six times with exponential
backoff from 250 ms to 4 s, and reports recovery. `on_error = "stop"` makes a
permanent failure stop the run; the default keeps the remaining devices alive.

## Manual validation

These are the commands to run when you want to check the physical result. Each
run restores on exit.

**Static colors**

```bash
./target/release/luma run --effect static --color '#ff0000' --duration 4
./target/release/luma run --effect static --color '#00ff00' --duration 4
```

Expected: the K617, all three G102 zones, and the ASUS keyboard show red, then
green. The K617 returns to its previous firmware preset after each run. The
G102 and ASUS go to their configured defaults.

**Wave coherence**

```bash
./target/release/luma run --effect wave --speed 2 --duration 6
```

Expected: a moving band on the K617, the same phase sampled at left, center,
and right on the G102, and a pulse on the single-zone ASUS. The K617 is the
most detailed view; the other two are capability downgrades of the same math.

**Focused K617 key slots**

```bash
./target/release/luma self-test --keys Escape,Menu,RCtrl
```

Expected: all keys are dim blue. Escape is bright white for 1.5 s, then Menu,
then RCtrl. The terminal prints the name before each hold. If a different key
lights, that is a slot-map difference in your keyboard variant. If no key
lights while the rest stays dim blue, run it with `--log debug` and report the
output.

**Full sweep**

```bash
./target/release/luma self-test --keymap
```

Expected: each of the 61 keys lights in row-major order for 300 ms against a
dim blue background. This is the quickest way to find a regional layout
difference.

**Everything at once**

```bash
./scripts/live-test.sh
```

The script mirrors its output to `luma-live-test.log`.

## Known limitations

* **K617 Menu and RCtrl slot positions are unverified.** The rest of the key
  map is hardware-verified upstream. Run `self-test --keys Menu,RCtrl` and, if
  needed, adjust the two entries in `src/device/k617.rs`.
* **K617 variants differ.** This map targets the US 61-key K617 Fizz. An ABNT2
  or other regional matrix may need a different slot table. The anchors and
  config protocol are shared, but the physical positions are not guaranteed.
* **K617 custom per-key presets are not part of the snapshot.** The 136-byte
  config contains the mode, color, speed, brightness, and start group, but not
  the five stored per-key color groups. If the keyboard was in custom mode 21
  before a run, the restored config may show the last stored group rather than
  the exact pre-run frame. We never write mode 21. For the common firmware
  effect case the restore is exact. If you depend on stored per-key presets,
  switch to a firmware effect before running or back up the profile with the
  vendor tool.
* **K617 per-key streaming is host-driven.** The keyboard's own 20 effects are
  not phase-locked to the engine. We stream frames at 30 fps instead. Very high
  frame rates are not verified.
* **G102 has three zones, not per-key.** The driver maps left, center, and
  right. It cannot read the current color, so exit restore is a configured
  default.
* **ASUS TUF A15 has one keyboard zone.** The `asus-wmi` ABI exposes one color
  for the whole keyboard. Effects are rendered at 15 fps to keep ACPI traffic
  reasonable.
* **The ASUS mode is write-only.** We restore the captured brightness and a
  configured color, not the exact previous effect. The sample uses white.
* **Windows is not implemented in v1.** A Windows ASUS backend would need the
  ATKACPI driver and a separate implementation. The G102 and K617 would work
  through the Windows hidapi backend with relatively little change, but that
  path is untested.
* **No tray or GUI.** The CLI is the supported interface. The session and
  engine layers are separated so a daemon or tray front end can be added
  without touching the drivers.
* **SIGKILL cannot restore.** Use Ctrl-C, SIGTERM, or `--duration` for a clean
  shutdown.

## Troubleshooting

**`permission denied opening /dev/hidraw1`**

Install the rule, then replug:

```bash
sudo ./scripts/install-permissions.sh
# replug the mouse and keyboard
ls -l /dev/hidraw1 /dev/hidraw3
```

The nodes should be `root:input` with mode `0660` and an ACL for your user.
Check the resolved interface:

```bash
udevadm info -q property /sys/class/hidraw/hidraw1 | grep ID_USB_INTERFACE_NUM
```

**`no hidraw node for 046d:c092 ...`**

The mouse is not attached or the kernel has not created the vendor interface.
Check `lsusb`, try another port, and run `luma list-devices`.

**ASUS interface not found**

```bash
ls /sys/class/leds/asus::kbd_backlight/
dmesg | grep -i asus
```

The model needs `asus-wmi` TUF RGB support. On this FA506IHRB it is present in
kernel 7.3.1. Reboot after a kernel update if the attribute disappears.

**K617 stays dark after a run**

The direct frame path is volatile and the config restore writes the captured
mode back. If a run was killed with SIGKILL, the last frame can remain. The
recovery command opens the K617, reads its current config, and writes that
config back, which leaves a stale direct frame:

```bash
./target/release/luma restore
```

If that is not enough, send a visible static frame without restore:

```bash
./target/release/luma run --effect static --color '#ffffff' --duration 2 --no-restore
```

Then run `luma restore` or replug the keyboard. If the dark state repeats,
capture the snapshot decode line with `--log debug` and open an issue with the
mode number.

**K617 key test shows no single key**

Run the focused test and tell the tool which key actually lights:

```bash
./target/release/luma self-test --keys Escape,Menu,RCtrl
```

The background is dim blue and the target key is white. If a different key
lights, the slot map needs a one-line adjustment for your variant. If the
whole keyboard ignores the frame, include the `--log debug` output.

**G102 does not change**

Close G HUB or any other process that owns the mouse, then replug. The G102
uses the vendor direct-mode packet, so another program can fight it. Verify the
three zones with `self-test` and check that the mouse is on interface 1.

**Wave looks out of phase**

Run `cycle` first. If cycle matches but wave does not, the devices are being
sampled at different positions, which is expected for a single-zone laptop. If
cycle is also off, lower `--fps` to 20 and check the `late` counter. A device
that is consistently late needs a lower `max_fps` or a less aggressive effect.

**Build fails on hidapi**

Install a C compiler and pkg-config (`base-devel` on Arch). The default crate
feature builds the hidraw backend from source. No libusb is used.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run -- run --dry-run --effect wave --duration 2
./scripts/live-test.sh   # visible hardware test, writes luma-live-test.log
```

The live test covers discovery, the read-only probe, red/green on all devices,
wave, cycle, the color self-test, the K617 keymap sweep, and a final restore.

## License and attribution

luma is licensed under GPL-3.0-or-later. Protocol knowledge and packet
layouts were studied in OpenRGB (GPL-2.0-or-later), the fizz-rgb K617 project
(MIT), and redragon-fizz-k617-rgb (MIT). No source code was copied verbatim;
the wire formats and constants were reimplemented and tested here. See
[`THIRD_PARTY.md`](THIRD_PARTY.md) for links and license texts.
