//! Physical K617 layout and the Sinodragon LED slot table.
//!
//! The keyboard matrix is 16 columns by 6 rows. A switch at column `c` and
//! row `r` lives at slot `c * 6 + r`. The 60 percent layout leaves gaps in
//! the matrix, which is why the slot numbers are not contiguous.
//!
//! The anchors in the test below were verified on K617 hardware by the
//! fizz-rgb project. Menu and RCtrl are upstream best guesses and can be
//! checked with `luma self-test --keys Menu,RCtrl`.

use crate::geometry::LedSpec;

#[derive(Clone, Copy)]
struct KeyDef {
    name: &'static str,
    width: f32,
    slot: u8,
}

const ROW_0: &[KeyDef] = &[
    KeyDef {
        name: "Escape",
        width: 1.0,
        slot: 0,
    },
    KeyDef {
        name: "1",
        width: 1.0,
        slot: 7,
    },
    KeyDef {
        name: "2",
        width: 1.0,
        slot: 13,
    },
    KeyDef {
        name: "3",
        width: 1.0,
        slot: 19,
    },
    KeyDef {
        name: "4",
        width: 1.0,
        slot: 25,
    },
    KeyDef {
        name: "5",
        width: 1.0,
        slot: 31,
    },
    KeyDef {
        name: "6",
        width: 1.0,
        slot: 37,
    },
    KeyDef {
        name: "7",
        width: 1.0,
        slot: 43,
    },
    KeyDef {
        name: "8",
        width: 1.0,
        slot: 49,
    },
    KeyDef {
        name: "9",
        width: 1.0,
        slot: 55,
    },
    KeyDef {
        name: "0",
        width: 1.0,
        slot: 61,
    },
    KeyDef {
        name: "Minus",
        width: 1.0,
        slot: 67,
    },
    KeyDef {
        name: "Equal",
        width: 1.0,
        slot: 73,
    },
    KeyDef {
        name: "Backspace",
        width: 2.0,
        slot: 79,
    },
];

const ROW_1: &[KeyDef] = &[
    KeyDef {
        name: "Tab",
        width: 1.5,
        slot: 2,
    },
    KeyDef {
        name: "Q",
        width: 1.0,
        slot: 8,
    },
    KeyDef {
        name: "W",
        width: 1.0,
        slot: 14,
    },
    KeyDef {
        name: "E",
        width: 1.0,
        slot: 20,
    },
    KeyDef {
        name: "R",
        width: 1.0,
        slot: 26,
    },
    KeyDef {
        name: "T",
        width: 1.0,
        slot: 32,
    },
    KeyDef {
        name: "Y",
        width: 1.0,
        slot: 38,
    },
    KeyDef {
        name: "U",
        width: 1.0,
        slot: 44,
    },
    KeyDef {
        name: "I",
        width: 1.0,
        slot: 50,
    },
    KeyDef {
        name: "O",
        width: 1.0,
        slot: 56,
    },
    KeyDef {
        name: "P",
        width: 1.0,
        slot: 62,
    },
    KeyDef {
        name: "LBracket",
        width: 1.0,
        slot: 68,
    },
    KeyDef {
        name: "RBracket",
        width: 1.0,
        slot: 74,
    },
    KeyDef {
        name: "Backslash",
        width: 1.5,
        slot: 80,
    },
];

const ROW_2: &[KeyDef] = &[
    KeyDef {
        name: "CapsLock",
        width: 1.75,
        slot: 3,
    },
    KeyDef {
        name: "A",
        width: 1.0,
        slot: 9,
    },
    KeyDef {
        name: "S",
        width: 1.0,
        slot: 15,
    },
    KeyDef {
        name: "D",
        width: 1.0,
        slot: 21,
    },
    KeyDef {
        name: "F",
        width: 1.0,
        slot: 27,
    },
    KeyDef {
        name: "G",
        width: 1.0,
        slot: 33,
    },
    KeyDef {
        name: "H",
        width: 1.0,
        slot: 39,
    },
    KeyDef {
        name: "J",
        width: 1.0,
        slot: 45,
    },
    KeyDef {
        name: "K",
        width: 1.0,
        slot: 51,
    },
    KeyDef {
        name: "L",
        width: 1.0,
        slot: 57,
    },
    KeyDef {
        name: "Semicolon",
        width: 1.0,
        slot: 63,
    },
    KeyDef {
        name: "Quote",
        width: 1.0,
        slot: 69,
    },
    KeyDef {
        name: "Enter",
        width: 2.25,
        slot: 81,
    },
];

const ROW_3: &[KeyDef] = &[
    KeyDef {
        name: "LShift",
        width: 2.25,
        slot: 4,
    },
    KeyDef {
        name: "Z",
        width: 1.0,
        slot: 10,
    },
    KeyDef {
        name: "X",
        width: 1.0,
        slot: 16,
    },
    KeyDef {
        name: "C",
        width: 1.0,
        slot: 22,
    },
    KeyDef {
        name: "V",
        width: 1.0,
        slot: 28,
    },
    KeyDef {
        name: "B",
        width: 1.0,
        slot: 34,
    },
    KeyDef {
        name: "N",
        width: 1.0,
        slot: 40,
    },
    KeyDef {
        name: "M",
        width: 1.0,
        slot: 46,
    },
    KeyDef {
        name: "Comma",
        width: 1.0,
        slot: 52,
    },
    KeyDef {
        name: "Period",
        width: 1.0,
        slot: 58,
    },
    KeyDef {
        name: "Slash",
        width: 1.0,
        slot: 64,
    },
    KeyDef {
        name: "RShift",
        width: 2.75,
        slot: 82,
    },
];

const ROW_4: &[KeyDef] = &[
    KeyDef {
        name: "LCtrl",
        width: 1.25,
        slot: 5,
    },
    KeyDef {
        name: "LSuper",
        width: 1.25,
        slot: 11,
    },
    KeyDef {
        name: "LAlt",
        width: 1.25,
        slot: 17,
    },
    KeyDef {
        name: "Space",
        width: 6.25,
        slot: 35,
    },
    KeyDef {
        name: "RAlt",
        width: 1.25,
        slot: 53,
    },
    KeyDef {
        name: "Fn",
        width: 1.25,
        slot: 59,
    },
    KeyDef {
        name: "Menu",
        width: 1.25,
        slot: 65,
    },
    KeyDef {
        name: "RCtrl",
        width: 1.25,
        slot: 71,
    },
];

const ROWS: &[&[KeyDef]] = &[ROW_0, ROW_1, ROW_2, ROW_3, ROW_4];
pub(crate) fn build_layout() -> (Vec<LedSpec>, Vec<u8>) {
    let mut layout = Vec::with_capacity(61);
    let mut slots = Vec::with_capacity(61);
    for (row_index, row) in ROWS.iter().enumerate() {
        let total: f32 = row.iter().map(|key| key.width).sum();
        let mut column = 0.0;
        for key in *row {
            let x = (column + key.width / 2.0) / total;
            let y = (row_index as f32 + 0.5) / ROWS.len() as f32;
            layout.push(LedSpec::new(key.name, x, y));
            slots.push(key.slot);
            column += key.width;
        }
    }
    (layout, slots)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLOTS: usize = 126;

    #[test]
    fn layout_has_61_unique_in_range_slots() {
        let (layout, slots) = build_layout();
        assert_eq!(layout.len(), 61);
        assert_eq!(slots.len(), 61);
        let mut seen = std::collections::HashSet::new();
        for slot in &slots {
            assert!(*slot < SLOTS as u8);
            assert!(seen.insert(*slot), "duplicate slot {slot}");
        }
        for led in &layout {
            assert!((0.0..=1.0).contains(&led.position.x));
            assert!((0.0..=1.0).contains(&led.position.y));
        }
    }

    #[test]
    fn verified_anchor_slots_are_unchanged() {
        let (layout, slots) = build_layout();
        let find = |name: &str| {
            let index = layout.iter().position(|led| led.name == name).unwrap();
            slots[index]
        };
        assert_eq!(find("Escape"), 0);
        assert_eq!(find("Tab"), 2);
        assert_eq!(find("CapsLock"), 3);
        assert_eq!(find("LShift"), 4);
        assert_eq!(find("LCtrl"), 5);
        assert_eq!(find("Space"), 35);
        assert_eq!(find("J"), 45);
        assert_eq!(find("Enter"), 81);
    }
}
