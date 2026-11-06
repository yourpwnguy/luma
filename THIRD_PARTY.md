# Third-party attribution

luma is GPL-3.0-or-later. The following projects were used as protocol
references. Their code was not copied verbatim; the packet layouts, constants,
and key-slot data were reimplemented and tested in this repository.

## OpenRGB

<https://github.com/CalcProgrammer1/OpenRGB>

License: GPL-2.0-or-later. Used as a reference for the Logitech G203L direct
protocol and the ASUS TUF `kbd_rgb_mode` ABI. luma is GPL-3.0-or-later, which
is compatible with GPL-2.0-or-later.

## fizz-rgb

<https://github.com/MrSchrodingers/fizz-rgb>

License: MIT. Used as a reference for the K617 Sinodragon report `0x08` frame
and the physical LED slot mapping.

```text
MIT License

Copyright (c) 2026 Matheus Munhoz and fizz-rgb contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## redragon-fizz-k617-rgb

<https://github.com/MarceloAntonio/redragon-fizz-k617-rgb>

License: MIT. Used as a reference for the K617 136-byte config read/write
protocol.

```text
MIT License

Copyright (c) 2026 MarceloAntonio

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Linux kernel

<https://github.com/torvalds/linux>

License: GPL-2.0. Used as the source of truth for the `asus-wmi` sysfs ABI
(`kbd_rgb_mode_store` and `kbd_rgb_state_store`).
