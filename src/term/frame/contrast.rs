use std::collections::HashMap;

use gpui::{Hsla, Rgba};

const MINIMUM: f32 = 4.5;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct Key {
    ink: [u32; 3],
    ground: Option<[u32; 3]>,
}

pub(super) struct Contrast {
    ground: f32,
    colors: HashMap<Key, Hsla>,
}

impl Contrast {
    pub(super) fn new(ground: Hsla) -> Self {
        Self {
            ground: luminance(ground.to_rgb()),
            colors: HashMap::new(),
        }
    }

    pub(super) fn text(&mut self, ink: Hsla, ground: Option<Hsla>) -> Hsla {
        if ink.a < 1.0 || ground.is_some_and(|ground| ground.a < 1.0) {
            return ink;
        }
        let key_of = |c: Hsla| [c.h.to_bits(), c.s.to_bits(), c.l.to_bits()];
        let key = Key {
            ink: key_of(ink),
            ground: ground.map(key_of),
        };
        if let Some(&color) = self.colors.get(&key) {
            return color;
        }
        let ground = ground.map_or(self.ground, |c| luminance(c.to_rgb()));
        let rgb = ink.to_rgb();
        let color = if ratio(luminance(rgb), ground) >= MINIMUM {
            ink
        } else {
            let target = if ratio(ground, 0.0) >= ratio(ground, 1.0) {
                0.0
            } else {
                1.0
            };
            let mix = |amount: f32| {
                let mix = |channel: f32| channel + (target - channel) * amount;
                Rgba {
                    r: mix(rgb.r),
                    g: mix(rgb.g),
                    b: mix(rgb.b),
                    a: rgb.a,
                }
            };
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..12 {
                let mid = (lo + hi) / 2.0;
                if ratio(luminance(mix(mid)), ground) >= MINIMUM {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            mix(hi).into()
        };
        if self.colors.len() < 256 {
            self.colors.insert(key, color);
        }
        color
    }
}

fn luminance(rgb: Rgba) -> f32 {
    let linear = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(rgb.r) + 0.7152 * linear(rgb.g) + 0.0722 * linear(rgb.b)
}

fn ratio(a: f32, b: f32) -> f32 {
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_fixed_pink_is_adjusted_on_light_and_preserved_on_dark() {
        let pink = gpui::rgb(0xdd7878).into();
        let light = gpui::rgb(0xfffdfd).into();
        let dark = gpui::rgb(0x1e1e2e).into();
        let adjusted = Contrast::new(light).text(pink, None);
        assert_ne!(adjusted, pink);
        assert!(ratio(luminance(adjusted.to_rgb()), luminance(light.to_rgb())) >= MINIMUM);
        assert_eq!(Contrast::new(dark).text(pink, None), pink);
    }

    #[test]
    fn an_existing_frame_changes_with_the_theme_without_rewriting_terminal_colors() {
        use crate::{
            term::{frame::frame, ink::Ink},
            themes,
        };
        use alacritty_terminal::{
            event::VoidListener,
            term::{Config, Term, test::TermSize},
            vte::ansi::Processor,
        };

        let mut term = Term::new(Config::default(), &TermSize::new(12, 3), VoidListener);
        let mut parser: Processor = Processor::new();
        parser.advance(&mut term, b"\x1b[38;2;221;120;120mP\x1b[44mB\x1b[49;7mI");
        let draw = |id, adaptive| {
            let theme = themes::find(id).expect("built-in palette");
            frame(
                &term,
                &Ink::new(theme, term.colors()).with_adaptive_colors(adaptive),
                &[],
                None,
            )
        };
        let light = draw("end4-kde-light", true);
        let exact = draw("end4-kde-light", false);
        let dark = draw("end4-kde-dark", true);
        assert_eq!(light.text(), "PBI");
        assert_ne!(light.rows[0].runs[0].1.fg, exact.rows[0].runs[0].1.fg);
        assert_eq!(dark.rows[0].runs[0].1.fg, exact.rows[0].runs[0].1.fg);
        assert_eq!(light.rows[0].runs[1].1, exact.rows[0].runs[1].1);
        let (shown, original) = (light.rows[0].runs[2].1, exact.rows[0].runs[2].1);
        assert_eq!(shown.bg, original.bg);
        assert_ne!(shown.fg, original.fg);
        assert!(
            ratio(
                luminance(shown.fg.to_rgb()),
                luminance(shown.bg.unwrap().to_rgb())
            ) >= MINIMUM
        );
    }

    #[test]
    fn a_dark_color_lightens_and_intentional_dim_alpha_stays() {
        let dark = gpui::rgb(0x1e1e2e).into();
        let dim = Hsla::from(gpui::rgb(0x45475a));
        let brighter = Contrast::new(dark).text(dim, None);
        assert!(brighter.l > dim.l);
        let translucent = dim.opacity(0.66);
        assert_eq!(Contrast::new(dark).text(translucent, None), translucent);
    }
}
