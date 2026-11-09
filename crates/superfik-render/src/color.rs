//! sRGB colors. Blending happens in gamma space, matching design tools.

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    pub const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const WHITE: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

    pub const fn hex(rgb: u32) -> Self {
        Self {
            r: ((rgb >> 16) & 0xff) as f32 / 255.0,
            g: ((rgb >> 8) & 0xff) as f32 / 255.0,
            b: (rgb & 0xff) as f32 / 255.0,
            a: 1.0,
        }
    }

    pub const fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    pub fn faded(self, opacity: f32) -> Self {
        Self { a: self.a * opacity.clamp(0.0, 1.0), ..self }
    }

    pub fn mix(self, other: Color, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    pub fn hsl(hue: f32, saturation: f32, lightness: f32) -> Self {
        let c = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
        let rgb = |n: f32| {
            let k = (n + hue.rem_euclid(1.0) * 12.0).rem_euclid(12.0);
            lightness - c * 0.5 * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0)
        };
        Self { r: rgb(0.0), g: rgb(8.0), b: rgb(4.0), a: 1.0 }
    }

    pub const fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    pub fn to_wgpu(self) -> wgpu::Color {
        wgpu::Color { r: self.r as f64, g: self.g as f64, b: self.b as f64, a: self.a as f64 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex() {
        let c = Color::hex(0x3a3a3a);
        assert!((c.r - 58.0 / 255.0).abs() < 1e-6);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn hsl_hits_primaries_and_greys() {
        let red = Color::hsl(0.0, 1.0, 0.5);
        assert!((red.r - 1.0).abs() < 1e-5 && red.g.abs() < 1e-5 && red.b.abs() < 1e-5);
        let green = Color::hsl(1.0 / 3.0, 1.0, 0.5);
        assert!((green.g - 1.0).abs() < 1e-5 && green.r.abs() < 1e-5);
        let grey = Color::hsl(0.7, 0.0, 0.4);
        assert!((grey.r - 0.4).abs() < 1e-5 && (grey.b - 0.4).abs() < 1e-5);
        assert_eq!(Color::hsl(1.25, 0.5, 0.5), Color::hsl(0.25, 0.5, 0.5));
    }

    #[test]
    fn mixes_and_fades() {
        let mid = Color::BLACK.mix(Color::WHITE, 0.5);
        assert!((mid.g - 0.5).abs() < 1e-6);
        assert_eq!(Color::WHITE.faded(0.25).a, 0.25);
        assert_eq!(Color::WHITE.faded(3.0).a, 1.0);
    }
}
