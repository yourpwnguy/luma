# Protocol notes and evidence

This file separates four kinds of claims:

* **CONFIRMED (live)**: exercised against the hardware attached to the
  development machine on 2026-11-06.
* **CONFIRMED (upstream)**: the packet layout is documented and tested by an
  upstream project on the same device family, but this repository did not
  independently verify the bytes on the wire.
* **INFERRED**: a conclusion drawn from a descriptor or a capture, but not yet
  exercised end to end.
* **UNKNOWN**: do not guess. Capture it first.

If a protocol detail is not in this file, do not invent it in the driver.

## Redragon K617 Fizz, `258a:0049`

### Identity

```text
Bus 003 Device 003: ID 258a:0049 BY Tech Gaming Keyboard
bcdDevice 1.03
Interface 0: HID boot keyboard, interrupt IN, report 1
Interface 1: vendor HID, report IDs 2 through 8
```

The driver opens interface 1 only. The report descriptor on that interface is
readable at:

```text
/sys/class/hidraw/hidraw3/device/report_descriptor
```

Relevant feature reports from the descriptor:

| Report | Size including report ID | Use |
| --- | --- | --- |
| `0x05` | 6 bytes | short command, config read handshake |
| `0x06` | 1032 bytes | 136-byte config block read/write |
| `0x08` | 382 bytes | 126-slot LED frame |

Report `0x08` is the fast path. It is sent as a HID feature report
(`HIDIOCSFEATURE`, which is a USB `SET_REPORT`, `bmRequestType=0x21`,
`bRequest=0x09`, `wValue=0x0308`).

### Per-key frame, report `0x08`

```text
offset 0     0x08        report ID
offset 1     0x0A
offset 2     0x7A
offset 3     0x01
offset 4..   126 RGB triplets, 378 bytes, column-major 6-row matrix
total        382 bytes
```

The slot index is physical, not the order of the key names in the config. The
matrix is 16 columns by 6 rows. The LED slot is `column * 6 + row` for the
switch at that physical position. Some slots are unused because the 60 percent
layout has no switch there.

Hardware-verified anchors from the fizz-rgb project:

| Key | Slot | Column, row |
| --- | --- | --- |
| Escape | 0 | 0, 0 |
| Tab | 2 | 0, 2 |
| CapsLock | 3 | 0, 3 |
| LShift | 4 | 0, 4 |
| LCtrl | 5 | 0, 5 |
| Space | 35 | 5, 5 |
| J | 45 | 7, 3 |
| Enter | 81 | 13, 3 |

The full table in `src/device/k617_layout.rs` is derived from the same layout. The
`Menu` (65) and `RCtrl` (71) entries are the two upstream best guesses. They
are marked in the code and can be checked with:

```bash
./target/release/luma self-test --keys Menu,RCtrl
```

A successful frame write returned in 6.8 ms on average, and 59 frames were sent
in a two second run. The 30 fps rate is therefore confirmed live.

The background of the focused key test is dim blue and the target key is white.
An earlier version used a black background and sent a single frame per key. The
keyboard looked off because the K617 leaves direct mode when frames stop. The
test now streams at 30 fps like the engine. Keep that behavior if you change
the self-test.

### Config read and write, reports `0x05`/`0x06`

Read:

```text
1. send feature report 0x05: 05 83 B6 00 00 00
2. wait about 50 ms
3. GET_FEATURE report 0x06 into a 1032-byte buffer with buf[0] = 0x06
4. response must have buf[1] == 0x83
5. the 136-byte config starts at offset 8
```

Write:

```text
buffer[0..3] = 06 03 B6
buffer[8..144] = the 136-byte config
all other bytes zero
total 1032 bytes, send as feature report 0x06
```

This is the only config write the driver performs, and only during exit
restore. Normal frames never touch the config.

The snapshot captured from the attached K617 decoded as:

```text
custom=0 mode=17 start_group=32 color_idx=7 speed_bright=0x52 magic=5aa5
```

Mode 17 is one of the firmware effects, color index 7 means random, the speed
nibble is 5, the brightness nibble is 2, and `5a a5` is the config magic. The
keyboard came back to its wave preset after the run, which confirms the write
path.

Modes 1 through 20 are firmware effects. Mode 21 is the stored custom per-key
mode. The driver does not write mode 21; it streams report `0x08` frames
instead.

### K617 safety boundary

The Sinowealth ISP bootloader and flash tools use separate sequences and
report IDs. luma never sends a bootloader entry, erase, flash, or verify
command. It only sends:

* report `0x08` frames while running,
* report `0x05` as the config read handshake at startup,
* report `0x06` to read the config and to restore it at exit.

If you extend this driver, keep that list closed unless you have a verified,
reversible reason to add a command.

### Sources

* fizz-rgb protocol notes:
  <https://github.com/MrSchrodingers/fizz-rgb/blob/main/docs/reverse-engineering/protocol.md>
* fizz-rgb per-key encoder:
  <https://github.com/MrSchrodingers/fizz-rgb/blob/main/packages/core/src/protocol.ts>
* fizz-rgb key layout:
  <https://github.com/MrSchrodingers/fizz-rgb/blob/main/packages/core/src/layout.ts>
* redragon-fizz-k617-rgb config read/write (MIT):
  <https://github.com/MarceloAntonio/redragon-fizz-k617-rgb/blob/main/src/lib.rs>
* KS82-B Sinowealth 382-byte report (MIT):
  <https://github.com/kn4oqw-clint/redragon-ks82b-rgb/blob/main/ks82rgb/controller.py>

## Logitech G102 LIGHTSYNC, `046d:c092`

### Identity

```text
Bus 003 Device 002: ID 046d:c092 Logitech, Inc. G102/G203 LIGHTSYNC Gaming Mouse
Interface 0: mouse, report 0x01
Interface 1: HID++ reports 0x10 and 0x11, 20-byte writes
```

The driver opens interface 1. All writes are 20 bytes including the report ID.
The mouse has three logical lighting zones, addressed as hardware LED IDs 1, 2,
and 3 (left, center, right).

### Packets

Enable software control at open:

```text
11 FF 0E 50 01 03 07 00 00 00 00 00 00 00 00 00 00 00 00 00
```

Set all three zones:

```text
11 FF 12 10 01 R G B 02 R G B 03 R G B FF 00 00 00
```

Apply:

```text
11 FF 12 70 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
```

The layout is three zones, not per-key. The driver sends the direct packet and
then the apply packet. On the first frame it repeats the pair because the
G203L firmware sometimes ignores the first direct update after the mode switch,
a workaround also present in OpenRGB's G203L driver.

### Readback

There is no verified read command for the current direct colors. Exit restore
therefore applies the configured default (white at 30 percent in the sample
config) or off. Do not invent a `GET` packet for this driver.

The read-only HID++ probe in this repository uses the standard HID++ 2.0 root
feature (`0x0000`) to enumerate the feature table. On this firmware it returned
only index `0x1e -> 0x0740`. The lighting interface is the legacy G203L feature
index `0x0e`, which is not exposed in the root table. The probe is diagnostic
only and is not used to drive the mouse.

### Sources

* OpenRGB G203L controller:
  <https://github.com/CalcProgrammer1/OpenRGB/blob/master/Controllers/LogitechController/LogitechG203LController/LogitechG203LController.cpp>
* OpenRGB G203L detector:
  <https://github.com/CalcProgrammer1/OpenRGB/blob/master/Controllers/LogitechController/LogitechControllerDetect.cpp>
* OpenRGB G203L RGB controller:
  <https://github.com/CalcProgrammer1/OpenRGB/blob/master/Controllers/LogitechController/LogitechG203LController/RGBController_LogitechG203L.cpp>

## ASUS TUF Gaming A15 FA506IHRB keyboard backlight

### Identity

```text
DMI vendor:  ASUSTeK COMPUTER INC.
DMI product: ASUS TUF Gaming A15 FA506IHRB_FA506IHRZ
DMI board:   FA506IHRB
BIOS:        FA506IHRB.307, 2022-12-28
```

The kernel exposes the keyboard backlight under:

```text
/sys/class/leds/asus::kbd_backlight/
```

Relevant files:

```text
kbd_rgb_mode           write-only, six decimal fields
kbd_rgb_mode_index     "cmd mode red green blue speed"
kbd_rgb_state          write-only, five decimal fields
kbd_rgb_state_index    "cmd boot awake sleep shutdown"
brightness             read/write, 0..max_brightness
max_brightness         3 on this model
```

The driver writes a single line to `kbd_rgb_mode`:

```text
0 0 R G B 1
```

The kernel's `kbd_rgb_mode_store()` in
`drivers/platform/x86/asus-wmi.c` parses six decimal integers with `sscanf`:

* `save` 0 maps to command `0xb3` (temporary, not saved to BIOS).
* `save` 1 maps to `0xb4` (save to BIOS). luma never sends 1.
* Mode 0 is static.
* Modes 3, 4, 5, 6, 7, 8, 10, 11 are available firmware effects. Mode 9 and
  values 12 or greater are coerced to 10.
* Speed 0 maps to `0xe1`, 1 to `0xeb`, 2 to `0xf5`. Static mode ignores it.

The eventual ACPI call is:

```text
ASUS_WMI_METHODID_DEVS, kbd_rgb_dev,
save_cmd | (mode << 8) | (r << 16) | (g << 24),
b | (speed << 8)
```

The `kbd_rgb_state` file controls boot/awake/sleep/shutdown lighting and is
persistent. luma never writes it.

The FA506 A15 has one keyboard zone. The sysfs ABI accepts one RGB triplet, so
the engine samples a single normalized position at the center of the keyboard.
Brightness is written through the standard LED class attribute, and the
original value is captured before the first change and restored at exit.

This is a hardware limit, not a missing driver. The internal keyboard is an
`AT Translated Set 2 keyboard` on `isa0060/serio0`, so it is a PS/2 device, not
a per-key RGB USB HID endpoint. `lsusb` on this machine shows no ASUS keyboard
RGB interface. The only exposed RGB control is the single-zone WMI command
above. There is no verified software-only path to address individual laptop
keys on this model. Per-key output would require a different keyboard module or
an external addressable LED matrix/strip with its own controller. Do not write
undocumented EC commands or flash the EC to attempt it.

Live result: the ACPI write changed the keyboard to red and green, and the
brightness went back to the original level after the run.

### Sources

* Linux `asus-wmi.c`, `kbd_rgb_mode_store()` and `kbd_rgb_state_store()`:
  <https://github.com/torvalds/linux/blob/master/drivers/platform/x86/asus-wmi.c>
* OpenRGB ASUS TUF Linux controller:
  <https://github.com/CalcProgrammer1/OpenRGB/blob/master/Controllers/AsusTUFLaptopController/RGBController_AsusTUFLaptop_Linux.cpp>
* OpenRGB ASUS TUF detection:
  <https://github.com/CalcProgrammer1/OpenRGB/blob/master/Controllers/AsusTUFLaptopController/AsusTUFLaptopDetect_Linux.cpp>

## Host-side effects

The `flow`, `orbit`, `chase`, `pulse`, and `pingpong` effects, plus the
full-field `aurora`, `constellation`, `circuit`, `ribbon`, and `bloom` effects,
do not add any device protocol. They only change the colors computed for each
frame. No new USB control transfers or sysfs writes are sent. The keyboard
still receives report `0x08` frames, the mouse still receives report `0x11`
packets, and the ASUS still receives the same `kbd_rgb_mode` line. The loop
mapping is described in [`ARCHITECTURE.md`](ARCHITECTURE.md).

## Live verification log

Date: 2026-11-06, Arch Linux, kernel 7.3.1-1-cachyos-lto.

```text
$ luma list-devices
g102      present   /dev/hidraw1 (interface 1, report 0x11)
k617      present   /dev/hidraw3 (interface 1, report 0x08)
asus_tuf  present   /sys/class/leds/asus::kbd_backlight (single zone, max brightness 3)

$ luma run --effect static --color '#ff0000' --duration 2
keyboard: 59f 6.9ms late=0 drop=0 err=0
mouse:    59f 3.3ms late=0 drop=0 err=0
laptop:   30f 0.0ms late=0 drop=29 err=0
all devices restored

$ luma run --effect static --color '#00ff00' --duration 2
keyboard: 59f 7.2ms late=0 drop=0 err=0
mouse:    59f 2.9ms late=0 drop=0 err=0
laptop:   30f 0.0ms late=0 drop=29 err=0
all devices restored
```

Visual confirmation from the machine owner: all three devices showed red and
green correctly. The K617 returned to its previous wave preset after each run,
the G102 went to the configured dim white, and the ASUS returned to white at
the captured brightness.

The five full-field effects were then run for six seconds each. Aurora, the
first of the five, reported:

```text
keyboard: 179f 6.6ms late=0 drop=0 err=0
mouse:    179f 3.4ms late=0 drop=0 err=0
laptop:    90f 0.6ms late=0 drop=89 err=0
all devices restored
```

The remaining four effects produced the same frame health: zero late frames and
zero write errors on the keyboard and mouse, with the laptop dropping alternate
frames by design because it runs at 15 fps. All runs restored the devices.

The focused key test (`self-test --keys Escape,Menu,RCtrl`) writes the frames
successfully but the physical slot confirmation for Menu and RCtrl is still
pending on this unit. The rest of the key map is confirmed upstream.

## Capturing an unknown device

Do not guess a packet. Capture the vendor software first, then replay one
change at a time with a restore path. A safe workflow:

1. Identify the USB identity and interfaces:
   `lsusb -v -d VID:PID`, `udevadm info -a -p /sys/class/hidraw/hidrawN`.
2. Read the HID report descriptor:
   `xxd /sys/class/hidraw/hidrawN/device/report_descriptor`.
   Note every report ID, its size, and whether it is Input, Output, or Feature.
3. On Linux, load `usbmon`, capture the bus in Wireshark, and filter by the
   device address. On Windows, install USBPcap and Wireshark and capture the
   root hub.
4. Run the vendor application in a VM with a snapshot if possible. Change one
   thing at a time: static red, static green, static blue, brightness, off.
5. In the capture, keep only `SET_REPORT` and `GET_REPORT` control transfers
   for the vendor interface. Record `bmRequestType`, `bRequest`, `wValue`,
   `wIndex`, `wLength`, and the full payload.
6. Diff the red, green, and blue captures to locate the RGB bytes. Do not
   assume RGB order. Verify with three primary colors.
7. Replay only the lighting packets. Never replay packets that look like
   firmware update, erase, bootloader entry, or flash. If a capture contains a
   bootloader magic, discard the whole sequence and start from a lighting-only
   action.
8. Keep the original packet lengths exact. `HIDIOCSFEATURE` encodes the length
   in the ioctl request; a short buffer is a different command.
9. Capture and restore the previous state if the protocol has a readback.
   Otherwise, configure a known default and document that the previous state
   is lost.
10. Add the driver as one file, register it, add a mock layout, and mark every
    claim CONFIRMED or INFERRED in this file.

A cheap powered USB hub with per-port power switching makes recovery from a
misbehaving device much less dramatic. It is not a substitute for avoiding
unknown firmware commands.
