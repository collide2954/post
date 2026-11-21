#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub background: Rgb,
    pub rule: Rgb,
    pub text: Rgb,
    pub muted: Rgb,
    pub accent: Rgb,
    pub on_accent: Rgb,
    pub danger: Rgb,
    pub selection: Rgb,
    pub handles: [Rgb; 8],
}

const LIGHT: Palette = Palette {
    background: Rgb(0xFB, 0xF8, 0xF3),
    rule: Rgb(0xDC, 0xD5, 0xC8),
    text: Rgb(0x1F, 0x23, 0x28),
    muted: Rgb(0x6A, 0x64, 0x59),
    accent: Rgb(0x24, 0x56, 0xA6),
    on_accent: Rgb(0xFB, 0xF8, 0xF3),
    danger: Rgb(0xB3, 0x26, 0x1E),
    selection: Rgb(0xCF, 0xE0, 0xF7),
    handles: [
        Rgb(0x1F, 0x5F, 0xAE),
        Rgb(0x0F, 0x6F, 0x6A),
        Rgb(0x2F, 0x6E, 0x2B),
        Rgb(0x6A, 0x64, 0x11),
        Rgb(0xA0, 0x52, 0x0F),
        Rgb(0xB0, 0x3A, 0x2E),
        Rgb(0x9B, 0x2F, 0x7A),
        Rgb(0x5E, 0x46, 0xB0),
    ],
};

const DARK: Palette = Palette {
    background: Rgb(0x1B, 0x1D, 0x21),
    rule: Rgb(0x34, 0x37, 0x3D),
    text: Rgb(0xE8, 0xE4, 0xDC),
    muted: Rgb(0x9C, 0x96, 0x8B),
    accent: Rgb(0x7F, 0xA8, 0xE8),
    on_accent: Rgb(0x1B, 0x1D, 0x21),
    danger: Rgb(0xF2, 0x87, 0x7E),
    selection: Rgb(0x2F, 0x44, 0x68),
    handles: [
        Rgb(0x7F, 0xB0, 0xF0),
        Rgb(0x5C, 0xC4, 0xBB),
        Rgb(0x8C, 0xCB, 0x7E),
        Rgb(0xC4, 0xBC, 0x5A),
        Rgb(0xE9, 0xA4, 0x6A),
        Rgb(0xF0, 0x90, 0x8A),
        Rgb(0xE0, 0x8C, 0xC6),
        Rgb(0xB3, 0xA1, 0xF2),
    ],
};

impl Palette {
    #[must_use]
    pub fn of(mode: Mode) -> Self {
        match mode {
            Mode::Light => LIGHT,
            Mode::Dark => DARK,
        }
    }

    #[must_use]
    pub fn handle(&self, name: &str) -> Rgb {
        self.handles[handle_index(name)]
    }
}

#[must_use]
pub fn handle_index(name: &str) -> usize {
    const FNV_OFFSET: u32 = 2_166_136_261;
    const FNV_PRIME: u32 = 16_777_619;

    let hash = name.bytes().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(FNV_PRIME)
    });
    (hash % 8) as usize
}

#[must_use]
pub fn contrast(first: Rgb, second: Rgb) -> f64 {
    let first = luminance(first);
    let second = luminance(second);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

fn luminance(color: Rgb) -> f64 {
    let linear = |channel: u8| {
        let value = f64::from(channel) / 255.0;
        if value <= 0.040_45 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.0) + 0.7152 * linear(color.1) + 0.0722 * linear(color.2)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{Mode, Palette, Rgb, contrast, handle_index};

    const WHITE: Rgb = Rgb(255, 255, 255);
    const BLACK: Rgb = Rgb(0, 0, 0);
    const TEXT_MINIMUM: f64 = 4.5;

    fn palettes() -> [(&'static str, Palette); 2] {
        [
            ("light", Palette::of(Mode::Light)),
            ("dark", Palette::of(Mode::Dark)),
        ]
    }

    fn near(left: f64, right: f64) -> bool {
        (left - right).abs() < 0.01
    }

    #[test]
    fn calculates_the_contrast_between_two_colors() {
        assert!(near(contrast(BLACK, WHITE), 21.0));
        assert!(near(contrast(WHITE, BLACK), 21.0));
        assert!(near(contrast(WHITE, WHITE), 1.0));
        assert!(near(contrast(Rgb(0x76, 0x76, 0x76), WHITE), 4.54));
        assert!(near(contrast(Rgb(255, 0, 0), WHITE), 4.0));
        assert!(near(contrast(Rgb(0, 255, 0), BLACK), 15.3));
        assert!(near(contrast(Rgb(0, 0, 255), WHITE), 8.59));
        assert!(near(contrast(Rgb(10, 10, 10), BLACK), 1.06));
    }

    #[test]
    fn each_text_color_is_easy_to_read_on_the_background() {
        for (mode, palette) in palettes() {
            let colors = [
                ("text", palette.text),
                ("muted", palette.muted),
                ("accent", palette.accent),
                ("danger", palette.danger),
            ];
            for (role, color) in colors {
                let ratio = contrast(color, palette.background);
                assert!(ratio >= TEXT_MINIMUM, "{mode} {role}: {ratio:.2}");
            }
            for (index, handle) in palette.handles.iter().enumerate() {
                let ratio = contrast(*handle, palette.background);
                assert!(ratio >= TEXT_MINIMUM, "{mode} handle {index}: {ratio:.2}");
            }
        }
    }

    #[test]
    fn the_main_text_has_a_very_high_contrast() {
        for (mode, palette) in palettes() {
            let ratio = contrast(palette.text, palette.background);
            assert!(ratio >= 7.0, "{mode}: {ratio:.2}");
        }
    }

    #[test]
    fn the_text_on_an_accent_or_a_selection_is_easy_to_read() {
        for (mode, palette) in palettes() {
            let on_accent = contrast(palette.on_accent, palette.accent);
            assert!(
                on_accent >= TEXT_MINIMUM,
                "{mode} on accent: {on_accent:.2}"
            );
            let on_selection = contrast(palette.text, palette.selection);
            assert!(
                on_selection >= TEXT_MINIMUM,
                "{mode} on selection: {on_selection:.2}"
            );
        }
    }

    #[test]
    fn a_rule_is_a_soft_line_that_a_person_can_see() {
        for (mode, palette) in palettes() {
            let ratio = contrast(palette.rule, palette.background);
            assert!((1.2..3.0).contains(&ratio), "{mode}: {ratio:.2}");
        }
    }

    #[test]
    fn light_mode_is_paper_and_dark_mode_is_ink() {
        let light = Palette::of(Mode::Light);
        let dark = Palette::of(Mode::Dark);

        assert!(contrast(light.background, WHITE) < 1.2);
        assert!(contrast(dark.background, BLACK) < 1.3);
        assert!(light.background.0 > light.background.2, "the paper is warm");
        assert_ne!(light, dark);
    }

    #[test]
    fn the_handle_colors_of_a_mode_are_all_different() {
        for (mode, palette) in palettes() {
            let different: BTreeSet<(u8, u8, u8)> = palette
                .handles
                .iter()
                .map(|color| (color.0, color.1, color.2))
                .collect();
            assert_eq!(different.len(), palette.handles.len(), "{mode}");
        }
    }

    #[test]
    fn a_name_always_gets_the_same_handle_color() {
        assert_eq!(handle_index(""), 5);
        assert_eq!(handle_index("a"), 4);
        assert_eq!(handle_index("foobar"), 0);

        let palette = Palette::of(Mode::Light);
        assert_eq!(palette.handle("a"), palette.handles[4]);
        assert_eq!(palette.handle("foobar"), palette.handles[0]);
    }

    #[test]
    fn different_names_use_all_the_handle_colors() {
        let used: BTreeSet<usize> = (0..64)
            .map(|number| handle_index(&format!("user-{number}")))
            .collect();
        assert_eq!(used.len(), 8);
        assert_eq!(used.into_iter().max(), Some(7));
    }
}
