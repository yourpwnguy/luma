#!/usr/bin/env python3
"""Live terminal mockups for five luma cross-device effects.

These five concepts are implemented in the Rust engine as `aurora`,
`constellation`, `circuit`, `ribbon`, and `bloom`. This script stays useful as
a no-hardware preview and a place to iterate on the animation math before
touching the real devices. The math here is approximate; the Rust modules are
the source of truth.

It draws a simplified keyboard (5 rows, 61 keys), a three-zone mouse, and a
single-zone laptop in the terminal, then animates the candidates.

Run it in a real terminal for color:

    ./scripts/preview-effects.py

Controls:
    1-5    switch effect
    space  pause or resume
    [ ]    slower or faster
    q      quit

If stdout is not a terminal, the script prints one static frame per effect
instead of animating. Use --snapshot to force that mode and --no-color for
plain ASCII output.
"""

from __future__ import annotations

import argparse
import math
import os
import select
import sys
import termios
import tty

TAU = math.tau
BLOCK = "\u2588"
RAMP = " .:-=+*#%@"

# Same loop slices as the real config, for an honest mock.
KEYBOARD = (0.00, 0.45)
MOUSE = (0.45, 0.70)
LAPTOP = (0.70, 0.85)

KEY_ROWS = [14, 14, 13, 12, 8]
EFFECTS = [
    (
        "aurora",
        "Aurora Field",
        (
            "Full-field teal and violet nebula. Every key stays lit while two "
            "soft glows drift across the keyboard and continue onto the mouse "
            "and laptop."
        ),
    ),
    (
        "constellation",
        "Constellation",
        (
            "Dim navy base with twinkling stars and a shooting star that "
            "crosses all three devices. The background never goes fully dark."
        ),
    ),
    (
        "circuit",
        "Circuit Flow",
        (
            "Teal circuit-board base with brighter traces and cyan data packets "
            "running keyboard -> mouse -> laptop."
        ),
    ),
    (
        "ribbon",
        "Prism Ribbon",
        (
            "A wide rainbow ribbon covers the whole keyboard. Brightness waves "
            "weave through it and carry the same color field onto the other "
            "devices."
        ),
    ),
    (
        "bloom",
        "Triad Bloom",
        (
            "Three colored glows anchored to the keyboard, mouse, and laptop. "
            "The glows overlap, mix, and pulse together like a heartbeat."
        ),
    ),
]


def clamp(value: float, low: float = 0.0, high: float = 255.0) -> float:
    return max(low, min(high, value))


def scale(
    color: tuple[float, float, float], factor: float
) -> tuple[float, float, float]:
    return (color[0] * factor, color[1] * factor, color[2] * factor)


def mix(
    a: tuple[float, float, float],
    b: tuple[float, float, float],
    amount: float,
) -> tuple[float, float, float]:
    amount = max(0.0, min(1.0, amount))
    return (
        a[0] + (b[0] - a[0]) * amount,
        a[1] + (b[1] - a[1]) * amount,
        a[2] + (b[2] - a[2]) * amount,
    )


def add(
    a: tuple[float, float, float],
    b: tuple[float, float, float],
) -> tuple[float, float, float]:
    return (a[0] + b[0], a[1] + b[1], a[2] + b[2])


def hsv(hue: float, saturation: float, value: float) -> tuple[float, float, float]:
    hue = hue % 1.0
    saturation = max(0.0, min(1.0, saturation))
    value = max(0.0, min(1.0, value))
    h = hue * 6.0
    c = value * saturation
    x = c * (1.0 - abs(h % 2.0 - 1.0))
    m = value - c
    if h < 1:
        r, g, b = c, x, 0.0
    elif h < 2:
        r, g, b = x, c, 0.0
    elif h < 3:
        r, g, b = 0.0, c, x
    elif h < 4:
        r, g, b = 0.0, x, c
    elif h < 5:
        r, g, b = x, 0.0, c
    else:
        r, g, b = c, 0.0, x
    return ((r + m) * 255.0, (g + m) * 255.0, (b + m) * 255.0)


def clamp_color(color: tuple[float, float, float]) -> tuple[int, int, int]:
    return (
        int(clamp(color[0])),
        int(clamp(color[1])),
        int(clamp(color[2])),
    )


def hash01(value: int) -> float:
    raw = math.sin(value * 12.9898 + 78.233) * 43758.5453
    return raw - math.floor(raw)


def loop_position(span: tuple[float, float], local_x: float) -> float:
    return span[0] + (span[1] - span[0]) * max(0.0, min(1.0, local_x))


def aurora(
    device: str, x: float, y: float, loop_s: float, t: float, index: int
) -> tuple[int, int, int]:
    wave = 0.5 + 0.5 * math.sin(TAU * (x * 0.9 + y * 0.6 - t * 0.06))
    swirl = 0.5 + 0.5 * math.sin(TAU * (loop_s * 1.7 + t * 0.045 + 0.25))
    brightness = 0.55 + 0.45 * max(wave, swirl)
    hue = (0.55 + 0.10 * math.sin(t * 0.08) + 0.18 * wave + 0.08 * loop_s) % 1.0
    teal = hsv(hue, 0.82, brightness)
    violet = hsv((hue + 0.42) % 1.0, 0.88, brightness * 0.85)
    blend = 0.5 + 0.5 * math.sin(TAU * (loop_s * 0.7 + t * 0.025))
    return clamp_color(mix(teal, violet, 0.35 + 0.65 * blend))


def constellation(
    device: str, x: float, y: float, loop_s: float, t: float, index: int
) -> tuple[int, int, int]:
    wash = 0.85 + 0.15 * math.sin(TAU * (loop_s * 0.8 + t * 0.04))
    color = scale((24.0, 42.0, 90.0), wash)

    seed = index + (0 if device == "keyboard" else 300 if device == "mouse" else 600)
    star_hash = hash01(seed)
    if star_hash > 0.70:
        twinkle = 0.5 + 0.5 * math.sin(t * 2.2 + star_hash * 30.0)
        star = (0.45 + 0.55 * twinkle) * (0.6 + 0.4 * star_hash)
        color = add(color, scale((190.0, 225.0, 255.0), star))

    # Two faint constellation lines, so the keyboard has a readable pattern.
    if abs(y - (0.15 + 0.55 * x)) < 0.05:
        color = add(color, scale((55.0, 105.0, 230.0), 0.20))
    if abs(y - 0.72) < 0.04 and x < 0.72:
        color = add(color, scale((55.0, 105.0, 230.0), 0.14))

    shooting_phase = (t * 0.18) % 1.0
    distance = (loop_s - shooting_phase) % 1.0
    shooting = max(0.0, 1.0 - distance / 0.12) ** 3
    color = add(color, scale((255.0, 255.0, 255.0), shooting))
    return clamp_color(color)


def circuit(
    device: str, x: float, y: float, loop_s: float, t: float, index: int
) -> tuple[int, int, int]:
    trace = (
        0.16
        if (index % 5 == 0 or (int(y * 5) % 2 == 0 and int(x * 7) % 3 == 0))
        else 0.0
    )
    row_wave = 0.5 + 0.5 * math.sin(TAU * (x * 2.5 - t * 0.7 + y * 0.8))
    brightness = 0.30 + trace + 0.18 * row_wave
    color = hsv(0.47 + 0.04 * math.sin(t * 0.2 + loop_s), 0.85, min(1.0, brightness))

    packet_phase = (t * 0.18) % 1.0
    distance = (loop_s - packet_phase) % 1.0
    packet = max(0.0, 1.0 - distance / 0.10) ** 3
    color = mix(color, (180.0, 255.0, 255.0), packet * 0.9)

    second_phase = (t * 0.23) % 1.0
    second_distance = abs(((x - second_phase + 0.5) % 1.0) - 0.5)
    second_packet = max(0.0, 1.0 - second_distance / 0.08) ** 3
    color = mix(color, (80.0, 255.0, 220.0), second_packet * 0.35)
    return clamp_color(color)


def ribbon(
    device: str, x: float, y: float, loop_s: float, t: float, index: int
) -> tuple[int, int, int]:
    hue = (0.02 + loop_s * 0.78 + t * 0.035) % 1.0
    weave = 0.5 + 0.5 * math.cos(TAU * (x * 1.8 + y * 0.7 - t * 0.32))
    brightness = 0.42 + 0.58 * weave
    color = hsv(hue, 0.85, brightness)
    highlight = 0.5 + 0.5 * math.cos(TAU * (x * 3.0 - y * 0.5 - t * 0.55))
    return clamp_color(mix(color, (255.0, 255.0, 255.0), 0.18 * highlight))


def bloom(
    device: str, x: float, y: float, loop_s: float, t: float, index: int
) -> tuple[int, int, int]:
    centers = (
        (0.225, (0.0, 190.0, 255.0)),
        (0.575, (255.0, 40.0, 160.0)),
        (0.775, (255.0, 160.0, 20.0)),
    )
    beat = 0.5 + 0.5 * math.sin(TAU * t * 0.65)
    acc = (0.0, 0.0, 0.0)
    total = 0.0
    for center, color in centers:
        distance = ((loop_s - center + 0.5) % 1.0) - 0.5
        glow = math.exp(-((distance / 0.30) ** 2)) * (0.55 + 0.45 * beat)
        acc = add(acc, scale(color, glow))
        total += glow
    weight = 1.0 / max(1.0, total)
    color = add((6.0, 6.0, 14.0), scale(acc, weight * 1.25))
    color = add(color, (10.0, 10.0, 18.0))
    return clamp_color(color)


EFFECT_FUNCS = {
    "aurora": aurora,
    "constellation": constellation,
    "circuit": circuit,
    "ribbon": ribbon,
    "bloom": bloom,
}


def paint(rgb: tuple[int, int, int], text: str, color: bool) -> str:
    if not color:
        luminance = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
        ramp_index = min(len(RAMP) - 1, int(luminance / 255.0 * len(RAMP)))
        return RAMP[ramp_index] * len(text)
    return f"\x1b[38;2;{rgb[0]};{rgb[1]};{rgb[2]}m{text}\x1b[0m"


def led_color(
    name: str,
    device: str,
    local_x: float,
    local_y: float,
    loop_s: float,
    t: float,
    index: int,
) -> tuple[int, int, int]:
    return EFFECT_FUNCS[name](device, local_x, local_y, loop_s, t, index)


def render_frame(
    name: str, index: int, t: float, color: bool, paused: bool, speed: float
) -> str:
    _, title, description = EFFECTS[index]
    lines: list[str] = []
    lines.append("LUMA effect mocks     1-5 switch   space pause   [ ] speed   q quit")
    lines.append("")

    keyboard_index = 0
    for row_number, count in enumerate(KEY_ROWS):
        y = row_number / (len(KEY_ROWS) - 1)
        line = "  "
        for key in range(count):
            x = key / max(1, count - 1)
            loop_s = loop_position(KEYBOARD, x)
            rgb = led_color(name, "keyboard", x, y, loop_s, t, keyboard_index)
            line += paint(rgb, BLOCK * 2, color) + " "
            keyboard_index += 1
        if row_number == 1:
            mouse_line = "  mouse: "
            for zone in range(3):
                x = zone / 2.0
                loop_s = loop_position(MOUSE, x)
                rgb = led_color(name, "mouse", x, 0.5, loop_s, t, 100 + zone)
                mouse_line += paint(rgb, BLOCK * 2, color) + " "
            line = line + "  " + mouse_line
        lines.append(line)

    lines.append("")
    laptop_loop = loop_position(LAPTOP, 0.5)
    laptop_rgb = led_color(name, "laptop", 0.5, 0.5, laptop_loop, t, 200)
    lines.append("  laptop: " + paint(laptop_rgb, BLOCK * 30, color))
    lines.append("")
    state = "PAUSED" if paused else f"{speed:.1f}x"
    lines.append(f"  Effect {index + 1}/5: {title}    [{state}]")
    lines.append(f"  {description}")
    return "\n".join(lines)


def snapshot(color: bool) -> None:
    for index, (name, title, description) in enumerate(EFFECTS):
        print(f"\n=== {index + 1}/5 {title} ({name}) ===")
        print(render_frame(name, index, 1.6, color, False, 1.0))


def main() -> int:
    parser = argparse.ArgumentParser(description="Preview five luma effect concepts")
    parser.add_argument(
        "--snapshot", action="store_true", help="print one static frame per effect"
    )
    parser.add_argument(
        "--no-color", action="store_true", help="use ASCII shading instead of truecolor"
    )
    parser.add_argument(
        "--fps", type=float, default=20.0, help="animation rate, default 20"
    )
    parser.add_argument(
        "--speed", type=float, default=1.0, help="initial speed multiplier"
    )
    args = parser.parse_args()

    interactive = sys.stdin.isatty() and sys.stdout.isatty()
    color = sys.stdout.isatty() and not args.no_color
    if args.snapshot or not interactive:
        snapshot(color)
        return 0

    fd = sys.stdin.fileno()
    old_settings = termios.tcgetattr(fd)
    speed = max(0.3, min(3.0, args.speed))
    paused = False
    selected = 0
    t = 0.0
    frame_time = 1.0 / max(1.0, args.fps)

    sys.stdout.write("\x1b[2J\x1b[?25l")
    try:
        tty.setcbreak(fd)
        while True:
            frame = render_frame(
                EFFECTS[selected][0], selected, t, color, paused, speed
            )
            sys.stdout.write("\x1b[H")
            for line in frame.splitlines():
                sys.stdout.write("\x1b[2K" + line + "\n")
            sys.stdout.flush()

            ready, _, _ = select.select([sys.stdin], [], [], frame_time)
            if ready:
                key = os.read(fd, 1).decode("utf-8", errors="ignore")
                if key == "q":
                    break
                if key in "12345":
                    selected = int(key) - 1
                elif key == " ":
                    paused = not paused
                elif key == "[":
                    speed = max(0.3, speed - 0.1)
                elif key == "]":
                    speed = min(3.0, speed + 0.1)
            if not paused:
                t += frame_time * speed
    except KeyboardInterrupt:
        pass
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, old_settings)
        sys.stdout.write("\x1b[?25h\n")
        sys.stdout.flush()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
