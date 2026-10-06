//! Native Minecraft skin atlas projection, independent of the desktop UI.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SkinModel {
    #[default]
    Classic,
    Slim,
}

use image::{Rgba, RgbaImage};

#[derive(Clone, Copy)]
struct V {
    x: f32,
    y: f32,
    z: f32,
}
#[derive(Clone, Copy)]
struct UV {
    u: f32,
    v: f32,
}
#[derive(Clone, Copy)]
struct Screen {
    x: f32,
    y: f32,
    z: f32,
    inv: f32,
}
struct Raster<'a> {
    texture: &'a RgbaImage,
    frame: RgbaImage,
    depth: Vec<f32>,
    yaw: f32,
    width: u32,
    height: u32,
}

impl Raster<'_> {
    fn point(&self, p: V) -> Option<Screen> {
        let (sin, cos) = self.yaw.sin_cos();
        let x = p.x * cos - p.z * sin;
        let z = p.x * sin + p.z * cos;
        let distance = 48. - z;
        if distance <= 1. {
            return None;
        }
        let inv = 1. / distance;
        let focal = self.height as f32 * 1.15;
        Some(Screen {
            x: self.width as f32 * 0.5 + x * focal * inv,
            y: self.height as f32 * 0.5 + (p.y + 4.) * focal * inv,
            z,
            inv,
        })
    }

    fn face(&mut self, vertices: [V; 4], rect: (u32, u32, u32, u32), flip: bool, overlay: bool) {
        let Some(points): Option<[Screen; 4]> = vertices
            .map(|p| self.point(p))
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .and_then(|p| p.try_into().ok())
        else {
            return;
        };
        let (u, v, w, h) = rect;
        let uv = if flip {
            [
                UV {
                    u: (u + w) as f32,
                    v: v as f32,
                },
                UV {
                    u: u as f32,
                    v: v as f32,
                },
                UV {
                    u: u as f32,
                    v: (v + h) as f32,
                },
                UV {
                    u: (u + w) as f32,
                    v: (v + h) as f32,
                },
            ]
        } else {
            [
                UV {
                    u: u as f32,
                    v: v as f32,
                },
                UV {
                    u: (u + w) as f32,
                    v: v as f32,
                },
                UV {
                    u: (u + w) as f32,
                    v: (v + h) as f32,
                },
                UV {
                    u: u as f32,
                    v: (v + h) as f32,
                },
            ]
        };
        self.triangle(
            [points[0], points[1], points[2]],
            [uv[0], uv[1], uv[2]],
            overlay,
        );
        self.triangle(
            [points[0], points[2], points[3]],
            [uv[0], uv[2], uv[3]],
            overlay,
        );
    }

    fn triangle(&mut self, p: [Screen; 3], uv: [UV; 3], overlay: bool) {
        let area = edge(p[0].x, p[0].y, p[1].x, p[1].y, p[2].x, p[2].y);
        if area.abs() < 0.01 {
            return;
        }
        let min_x = p
            .iter()
            .map(|p| p.x.floor() as i32)
            .min()
            .unwrap_or(0)
            .clamp(0, self.width as i32);
        let max_x = p
            .iter()
            .map(|p| p.x.ceil() as i32)
            .max()
            .unwrap_or(0)
            .clamp(0, self.width as i32);
        let min_y = p
            .iter()
            .map(|p| p.y.floor() as i32)
            .min()
            .unwrap_or(0)
            .clamp(0, self.height as i32);
        let max_y = p
            .iter()
            .map(|p| p.y.ceil() as i32)
            .max()
            .unwrap_or(0)
            .clamp(0, self.height as i32);
        for y in min_y..max_y {
            for x in min_x..max_x {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let a = edge(p[1].x, p[1].y, p[2].x, p[2].y, px, py) / area;
                let b = edge(p[2].x, p[2].y, p[0].x, p[0].y, px, py) / area;
                let c = 1. - a - b;
                if a < 0. || b < 0. || c < 0. {
                    continue;
                }
                let reciprocal = a * p[0].inv + b * p[1].inv + c * p[2].inv;
                let depth = (a * p[0].z * p[0].inv + b * p[1].z * p[1].inv + c * p[2].z * p[2].inv)
                    / reciprocal;
                let idx = y as usize * self.width as usize + x as usize;
                if depth <= self.depth[idx] {
                    continue;
                }
                let u = (a * uv[0].u * p[0].inv + b * uv[1].u * p[1].inv + c * uv[2].u * p[2].inv)
                    / reciprocal;
                let v = (a * uv[0].v * p[0].inv + b * uv[1].v * p[1].inv + c * uv[2].v * p[2].inv)
                    / reciprocal;
                let tx = (u.floor() as u32).min(self.texture.width() - 1);
                let ty = (v.floor() as u32).min(self.texture.height() - 1);
                let mut color = *self.texture.get_pixel(tx, ty);
                if color[3] == 0 {
                    continue;
                }
                if overlay {
                    let base = *self.frame.get_pixel(x as u32, y as u32);
                    color = blend(color, base);
                } else {
                    let brightness = if p[0].z + p[1].z + p[2].z > 0. {
                        1.
                    } else {
                        0.74
                    };
                    color[0] = (color[0] as f32 * brightness) as u8;
                    color[1] = (color[1] as f32 * brightness) as u8;
                    color[2] = (color[2] as f32 * brightness) as u8;
                }
                self.frame.put_pixel(x as u32, y as u32, color);
                self.depth[idx] = depth;
            }
        }
    }
}

fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (px - ax) * (by - ay) - (py - ay) * (bx - ax)
}
fn blend(top: Rgba<u8>, under: Rgba<u8>) -> Rgba<u8> {
    let a = top[3] as f32 / 255.;
    Rgba([
        (top[0] as f32 * a + under[0] as f32 * (1. - a)) as u8,
        (top[1] as f32 * a + under[1] as f32 * (1. - a)) as u8,
        (top[2] as f32 * a + under[2] as f32 * (1. - a)) as u8,
        255,
    ])
}

impl Raster<'_> {
    fn cuboid(&mut self, bounds: [V; 2], uv: (u32, u32), size: (u32, u32, u32), overlay: bool) {
        let [
            V {
                x: x0,
                y: y0,
                z: z0,
            },
            V {
                x: x1,
                y: y1,
                z: z1,
            },
        ] = bounds;
        let (u, v) = uv;
        let (w, h, d) = size;
        self.face(
            [
                V {
                    x: x0,
                    y: y0,
                    z: z1,
                },
                V {
                    x: x1,
                    y: y0,
                    z: z1,
                },
                V {
                    x: x1,
                    y: y1,
                    z: z1,
                },
                V {
                    x: x0,
                    y: y1,
                    z: z1,
                },
            ],
            (u + d, v + d, w, h),
            false,
            overlay,
        );
        self.face(
            [
                V {
                    x: x1,
                    y: y0,
                    z: z0,
                },
                V {
                    x: x0,
                    y: y0,
                    z: z0,
                },
                V {
                    x: x0,
                    y: y1,
                    z: z0,
                },
                V {
                    x: x1,
                    y: y1,
                    z: z0,
                },
            ],
            (u + w + 2 * d, v + d, w, h),
            false,
            overlay,
        );
        self.face(
            [
                V {
                    x: x0,
                    y: y0,
                    z: z0,
                },
                V {
                    x: x0,
                    y: y0,
                    z: z1,
                },
                V {
                    x: x0,
                    y: y1,
                    z: z1,
                },
                V {
                    x: x0,
                    y: y1,
                    z: z0,
                },
            ],
            (u, v + d, d, h),
            true,
            overlay,
        );
        self.face(
            [
                V {
                    x: x1,
                    y: y0,
                    z: z1,
                },
                V {
                    x: x1,
                    y: y0,
                    z: z0,
                },
                V {
                    x: x1,
                    y: y1,
                    z: z0,
                },
                V {
                    x: x1,
                    y: y1,
                    z: z1,
                },
            ],
            (u + d + w, v + d, d, h),
            false,
            overlay,
        );
        self.face(
            [
                V {
                    x: x0,
                    y: y0,
                    z: z0,
                },
                V {
                    x: x1,
                    y: y0,
                    z: z0,
                },
                V {
                    x: x1,
                    y: y0,
                    z: z1,
                },
                V {
                    x: x0,
                    y: y0,
                    z: z1,
                },
            ],
            (u + d, v, w, d),
            false,
            overlay,
        );
        self.face(
            [
                V {
                    x: x0,
                    y: y1,
                    z: z1,
                },
                V {
                    x: x1,
                    y: y1,
                    z: z1,
                },
                V {
                    x: x1,
                    y: y1,
                    z: z0,
                },
                V {
                    x: x0,
                    y: y1,
                    z: z0,
                },
            ],
            (u + d + w, v, w, d),
            false,
            overlay,
        );
    }
}

pub fn render(
    texture: &RgbaImage,
    model: SkinModel,
    yaw: f32,
    width: u32,
    height: u32,
) -> RgbaImage {
    let mut raster = Raster {
        texture,
        frame: RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0])),
        depth: vec![f32::NEG_INFINITY; (width * height) as usize],
        yaw,
        width,
        height,
    };
    let legacy = texture.height() == 32;
    // Draw the far-side limbs first; the z-buffer still chooses visible texels.
    let arm = if model == SkinModel::Slim && !legacy {
        3.
    } else {
        4.
    };
    raster.cuboid(
        [
            V {
                x: -4.,
                y: -12.,
                z: -2.,
            },
            V {
                x: 4.,
                y: 0.,
                z: 2.,
            },
        ],
        (16, 16),
        (8, 12, 4),
        false,
    );
    raster.cuboid(
        [
            V {
                x: -4.,
                y: 0.,
                z: -2.,
            },
            V {
                x: 0.,
                y: 12.,
                z: 2.,
            },
        ],
        if legacy { (0, 16) } else { (16, 48) },
        (4, 12, 4),
        false,
    );
    raster.cuboid(
        [
            V {
                x: 0.,
                y: 0.,
                z: -2.,
            },
            V {
                x: 4.,
                y: 12.,
                z: 2.,
            },
        ],
        (0, 16),
        (4, 12, 4),
        false,
    );
    raster.cuboid(
        [
            V {
                x: -4. - arm,
                y: -12.,
                z: -2.,
            },
            V {
                x: -4.,
                y: 0.,
                z: 2.,
            },
        ],
        if legacy { (40, 16) } else { (32, 48) },
        (arm as u32, 12, 4),
        false,
    );
    raster.cuboid(
        [
            V {
                x: 4.,
                y: -12.,
                z: -2.,
            },
            V {
                x: 4. + arm,
                y: 0.,
                z: 2.,
            },
        ],
        (40, 16),
        (arm as u32, 12, 4),
        false,
    );
    raster.cuboid(
        [
            V {
                x: -4.,
                y: -20.,
                z: -4.,
            },
            V {
                x: 4.,
                y: -12.,
                z: 4.,
            },
        ],
        (0, 0),
        (8, 8, 8),
        false,
    );
    if !legacy {
        raster.cuboid(
            [
                V {
                    x: -4.15,
                    y: -20.15,
                    z: -4.15,
                },
                V {
                    x: 4.15,
                    y: -12.,
                    z: 4.15,
                },
            ],
            (32, 0),
            (8, 8, 8),
            true,
        );
        raster.cuboid(
            [
                V {
                    x: -4.15,
                    y: -12.,
                    z: -2.15,
                },
                V {
                    x: 4.15,
                    y: 0.,
                    z: 2.15,
                },
            ],
            (16, 32),
            (8, 12, 4),
            true,
        );
        raster.cuboid(
            [
                V {
                    x: -4.15,
                    y: 0.,
                    z: -2.15,
                },
                V {
                    x: 0.,
                    y: 12.,
                    z: 2.15,
                },
            ],
            (0, 48),
            (4, 12, 4),
            true,
        );
        raster.cuboid(
            [
                V {
                    x: 0.,
                    y: 0.,
                    z: -2.15,
                },
                V {
                    x: 4.15,
                    y: 12.,
                    z: 2.15,
                },
            ],
            (0, 32),
            (4, 12, 4),
            true,
        );
        raster.cuboid(
            [
                V {
                    x: -4.15 - arm,
                    y: -12.,
                    z: -2.15,
                },
                V {
                    x: -4.,
                    y: 0.,
                    z: 2.15,
                },
            ],
            (48, 48),
            (arm as u32, 12, 4),
            true,
        );
        raster.cuboid(
            [
                V {
                    x: 4.,
                    y: -12.,
                    z: -2.15,
                },
                V {
                    x: 4.15 + arm,
                    y: 0.,
                    z: 2.15,
                },
            ],
            (40, 32),
            (arm as u32, 12, 4),
            true,
        );
    }
    raster.frame
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_rotation_keeps_every_visible_pixel_inside_the_stage() {
        let texture = RgbaImage::from_fn(64, 64, |x, y| Rgba([x as u8 * 3, y as u8 * 3, 60, 255]));
        let started = std::time::Instant::now();
        for model in [SkinModel::Classic, SkinModel::Slim] {
            for angle in 0..24 {
                let frame = render(
                    &texture,
                    model,
                    angle as f32 * std::f32::consts::TAU / 24.,
                    224,
                    384,
                );
                assert!(frame.pixels().any(|pixel| pixel[3] > 0));
                for (x, y, pixel) in frame
                    .enumerate_pixels()
                    .filter(|(_, _, pixel)| pixel[3] > 0)
                {
                    assert!(
                        x > 0 && x < 223 && y > 0 && y < 383,
                        "clipped at {x}, {y}: {pixel:?}"
                    );
                }
            }
        }
        println!("48 skin renders took {:?}", started.elapsed());
    }
}
