//! A small software rasterizer for the solid preview. Shipping our own keeps
//! the viewer working on every backend eframe can start on, which matters more
//! here than the speed a GPU path would buy for a few thousand triangles.

use piracutter::mesh::Tri;
use egui::{Color32, ColorImage, Pos2};

pub type V3 = [f32; 3];

/// Half the vertical field of view, in radians.
const HALF_FOV: f32 = 0.5;

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 1e-9 {
        scale(a, 1.0 / l)
    } else {
        [0.0, 0.0, 1.0]
    }
}

#[derive(Clone, Copy)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub target: V3,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: -1.1,
            pitch: 0.75,
            dist: 200.0,
            target: [0.0, 0.0, 0.0],
        }
    }
}

pub struct Basis {
    eye: V3,
    right: V3,
    up: V3,
    fwd: V3,
    focal: f32,
    cx: f32,
    cy: f32,
}

impl Camera {
    pub fn basis(&self, w: f32, h: f32) -> Basis {
        let (cy, sy) = (self.yaw.cos(), self.yaw.sin());
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        let fwd = norm([cy * cp, sy * cp, -sp]);
        let eye = sub(self.target, scale(fwd, self.dist));
        let right = norm(cross(fwd, [0.0, 0.0, 1.0]));
        let up = cross(right, fwd);
        Basis {
            eye,
            right,
            up,
            fwd,
            focal: (h * 0.5) / HALF_FOV.tan(),
            cx: w * 0.5,
            cy: h * 0.5,
        }
    }

    /// Pulls back just far enough for `bbox` to fit the viewport from the
    /// current angle, so a wide scene is not framed as if it were a sphere.
    pub fn frame(&mut self, bbox: (V3, V3), aspect: f32) {
        let (lo, hi) = bbox;
        self.target = scale(add(lo, hi), 0.5);
        let b = self.basis(aspect, 1.0);
        let tan_v = HALF_FOV.tan();
        let tan_h = tan_v * aspect.max(0.1);
        let mut dist: f32 = 10.0;
        for i in 0..8 {
            let corner = [
                if i & 1 == 0 { lo[0] } else { hi[0] },
                if i & 2 == 0 { lo[1] } else { hi[1] },
                if i & 4 == 0 { lo[2] } else { hi[2] },
            ];
            let v = sub(corner, self.target);
            let (x, y, z) = (dot(v, b.right), dot(v, b.up), dot(v, b.fwd));
            dist = dist.max(x.abs() / tan_h - z).max(y.abs() / tan_v - z);
        }
        self.dist = dist * 1.08;
    }
}

impl Basis {
    /// Slides the orbit target across the screen plane.
    pub fn pan(&self, target: V3, dx: f32, dy: f32) -> V3 {
        add(target, add(scale(self.right, dx), scale(self.up, dy)))
    }

    pub fn project(&self, p: V3, near: f32) -> Option<(Pos2, f32)> {
        let v = sub(p, self.eye);
        let z = dot(v, self.fwd);
        if z < near {
            return None;
        }
        let inv = self.focal / z;
        Some((
            Pos2::new(self.cx + dot(v, self.right) * inv, self.cy - dot(v, self.up) * inv),
            z,
        ))
    }
}

pub struct Part {
    pub name: &'static str,
    pub tris: Vec<Tri>,
    pub color: [f32; 3],
    pub offset: V3,
}

pub fn bbox<'a>(parts: impl IntoIterator<Item = &'a Part>) -> Option<(V3, V3)> {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    let mut any = false;
    for part in parts.into_iter() {
        for t in &part.tris {
            for v in t {
                any = true;
                for i in 0..3 {
                    lo[i] = lo[i].min(v[i] + part.offset[i]);
                    hi[i] = hi[i].max(v[i] + part.offset[i]);
                }
            }
        }
    }
    any.then_some((lo, hi))
}

/// Paints `parts` into an RGBA image, transparent where nothing was drawn so
/// the caller can lay it over a build plate.
pub fn rasterize<'a>(
    parts: impl IntoIterator<Item = &'a Part>,
    cam: &Camera,
    w: usize,
    h: usize,
) -> ColorImage {
    let b = cam.basis(w as f32, h as f32);
    let mut px = vec![0u8; w * h * 4];
    let mut depth = vec![f32::MAX; w * h];
    let near = 0.1;
    let key = norm([0.35, -0.5, 0.8]);

    for part in parts.into_iter() {
        for t in &part.tris {
            let world: Vec<V3> = t.iter().map(|v| add(*v, part.offset)).collect();
            let n = norm(cross(sub(world[1], world[0]), sub(world[2], world[0])));
            let centroid = scale(add(add(world[0], world[1]), world[2]), 1.0 / 3.0);
            if dot(n, sub(b.eye, centroid)) <= 0.0 {
                continue;
            }
            let Some(p0) = b.project(world[0], near) else { continue };
            let Some(p1) = b.project(world[1], near) else { continue };
            let Some(p2) = b.project(world[2], near) else { continue };

            let lambert = dot(n, key).max(0.0);
            let rim = 1.0 - dot(n, norm(sub(b.eye, centroid))).abs();
            let shade = 0.30 + 0.62 * lambert + 0.10 * rim * rim;
            let color = [
                (part.color[0] * shade).clamp(0.0, 255.0) as u8,
                (part.color[1] * shade).clamp(0.0, 255.0) as u8,
                (part.color[2] * shade).clamp(0.0, 255.0) as u8,
            ];
            fill(&mut px, &mut depth, w, h, [p0, p1, p2], color);
        }
    }
    ColorImage::from_rgba_unmultiplied([w, h], &px)
}

fn fill(
    px: &mut [u8],
    depth: &mut [f32],
    w: usize,
    h: usize,
    v: [(Pos2, f32); 3],
    color: [u8; 3],
) {
    let (a, b, c) = (v[0].0, v[1].0, v[2].0);
    let area = (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y);
    if area.abs() < 1e-6 {
        return;
    }
    let inv_area = 1.0 / area;
    let x0 = a.x.min(b.x).min(c.x).floor().max(0.0) as usize;
    let x1 = (a.x.max(b.x).max(c.x).ceil() as isize).clamp(0, w as isize) as usize;
    let y0 = a.y.min(b.y).min(c.y).floor().max(0.0) as usize;
    let y1 = (a.y.max(b.y).max(c.y).ceil() as isize).clamp(0, h as isize) as usize;
    // Depth is interpolated as 1/z, which is what stays linear in screen space.
    let (iz0, iz1, iz2) = (1.0 / v[0].1, 1.0 / v[1].1, 1.0 / v[2].1);

    for y in y0..y1 {
        for x in x0..x1 {
            let p = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
            let w0 = ((b.x - p.x) * (c.y - p.y) - (c.x - p.x) * (b.y - p.y)) * inv_area;
            let w1 = ((c.x - p.x) * (a.y - p.y) - (a.x - p.x) * (c.y - p.y)) * inv_area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let iz = w0 * iz0 + w1 * iz1 + w2 * iz2;
            if iz <= 0.0 {
                continue;
            }
            let z = 1.0 / iz;
            let i = y * w + x;
            if z >= depth[i] {
                continue;
            }
            depth[i] = z;
            px[i * 4] = color[0];
            px[i * 4 + 1] = color[1];
            px[i * 4 + 2] = color[2];
            px[i * 4 + 3] = 255;
        }
    }
}

/// Build plate lines on z = 0, for the caller to stroke behind the model.
pub fn plate_lines(cam: &Camera, w: f32, h: f32, extent: f32, step: f32) -> Vec<([Pos2; 2], Color32)> {
    let b = cam.basis(w, h);
    let n = (extent / step).ceil() as i32;
    let reach = n as f32 * step;
    let mut out = Vec::new();
    for i in -n..=n {
        let t = i as f32 * step;
        let major = i == 0;
        let color = if major {
            Color32::from_gray(96)
        } else {
            Color32::from_gray(58)
        };
        for seg in [
            [[t, -reach, 0.0], [t, reach, 0.0]],
            [[-reach, t, 0.0], [reach, t, 0.0]],
        ] {
            if let (Some((p, _)), Some((q, _))) = (b.project(seg[0], 1.0), b.project(seg[1], 1.0)) {
                out.push(([p, q], color));
            }
        }
    }
    out
}
