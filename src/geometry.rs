use image::{GrayImage, Luma};
use imageproc::contours::find_contours;
use imageproc::distance_transform::euclidean_squared_distance_transform;

/// Signed distance in pixels to the mask edge. Positive outside, negative inside.
pub fn sdf(mask: &GrayImage) -> Vec<f32> {
    let (w, h) = mask.dimensions();
    let mut inv = GrayImage::new(w, h);
    for (x, y, p) in mask.enumerate_pixels() {
        if p[0] == 0 {
            inv.put_pixel(x, y, Luma([255]));
        }
    }
    let d_out = euclidean_squared_distance_transform(mask);
    let d_in = euclidean_squared_distance_transform(&inv);
    d_out
        .pixels()
        .zip(d_in.pixels())
        .map(|(o, i)| (o[0].sqrt() - i[0].sqrt()) as f32)
        .collect()
}

pub struct Field {
    pub d: Vec<f32>,
    pub w: u32,
    pub h: u32,
}

impl Field {
    pub fn new(mask: &GrayImage) -> Self {
        let (w, h) = mask.dimensions();
        Field { d: sdf(mask), w, h }
    }

    /// Mask of everything within `level` mm of the shape, `level` negative for
    /// erosion. Tracing runs through the centres of the outermost pixels, so
    /// the curve lands about one pixel inside the level asked for; no threshold
    /// nudge fixes that, since the discrete field steps straight from -1 to +1.
    pub fn below(&self, level_mm: f32, ppm: f32) -> GrayImage {
        let t = level_mm * ppm;
        let mut out = GrayImage::new(self.w, self.h);
        for (i, &d) in self.d.iter().enumerate() {
            if d < t {
                out.put_pixel(i as u32 % self.w, i as u32 / self.w, Luma([255]));
            }
        }
        out
    }
}

pub fn intersect(a: &GrayImage, b: &GrayImage) -> GrayImage {
    let mut out = a.clone();
    for (x, y, p) in b.enumerate_pixels() {
        if p[0] == 0 {
            out.put_pixel(x, y, Luma([0]));
        }
    }
    out
}

/// Two foreground pixels meeting only at a corner make the traced contour touch
/// itself, which no ear-clipping triangulator can handle. Widening the corner
/// costs one pixel and keeps every polygon simple.
fn fill_diagonal_pinches(mask: &GrayImage) -> GrayImage {
    let (w, h) = mask.dimensions();
    let mut out = mask.clone();
    for _ in 0..2 {
        let src = out.clone();
        let on = |x: u32, y: u32| src.get_pixel(x, y)[0] > 0;
        let mut changed = false;
        for y in 0..h.saturating_sub(1) {
            for x in 0..w.saturating_sub(1) {
                let (a, b, c, d) = (on(x, y), on(x + 1, y), on(x, y + 1), on(x + 1, y + 1));
                if (a && d && !b && !c) || (b && c && !a && !d) {
                    out.put_pixel(x, y, Luma([255]));
                    out.put_pixel(x + 1, y, Luma([255]));
                    out.put_pixel(x, y + 1, Luma([255]));
                    out.put_pixel(x + 1, y + 1, Luma([255]));
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    out
}

pub type Ring = Vec<[f64; 2]>;

#[derive(Clone, Debug)]
pub struct Polygon {
    pub outer: Ring,
    pub holes: Vec<Ring>,
}

pub struct ContourOpts {
    pub ppm: f32,
    pub mirror: bool,
    pub smooth_iters: u32,
    pub simplify_mm: f32,
}

/// Every contour of `mask`, in mm, without nesting. Nesting is re-derived by
/// `assemble`, so rings from different masks can be combined into one solid.
pub fn rings(mask: &GrayImage, o: &ContourOpts) -> Vec<Ring> {
    let h = mask.height() as f64;
    let ppm = o.ppm as f64;
    let mask = fill_diagonal_pinches(mask);
    find_contours::<i32>(&mask)
        .iter()
        .filter_map(|c| {
            let mut ring: Ring = c
                .points
                .iter()
                .map(|p| {
                    let x = p.x as f64 / ppm;
                    let y = if o.mirror { p.y as f64 / ppm } else { (h - p.y as f64) / ppm };
                    [x, y]
                })
                .collect();
            if ring.len() < 3 {
                return None;
            }
            remove_spurs(&mut ring);
            for _ in 0..o.smooth_iters {
                ring = chaikin(&ring);
            }
            ring = rdp_closed(&ring, o.simplify_mm as f64);
            dedup(&mut ring);
            (ring.len() >= 3 && signed_area(&ring).abs() > 1e-6).then_some(ring)
        })
        .collect()
}

/// Groups rings into polygons by containment: a ring nested an even number of
/// levels deep bounds material, an odd one bounds a hole.
pub fn assemble(rings: Vec<Ring>) -> Vec<Polygon> {
    let n = rings.len();
    let areas: Vec<f64> = rings.iter().map(|r| signed_area(r).abs()).collect();
    let mut parent: Vec<Option<usize>> = vec![None; n];
    for i in 0..n {
        let p = rings[i][0];
        for j in 0..n {
            if i != j && areas[j] > areas[i] && point_in_ring(p, &rings[j]) {
                if parent[i].map_or(true, |b| areas[j] < areas[b]) {
                    parent[i] = Some(j);
                }
            }
        }
    }

    let depth = |mut i: usize| {
        let mut d = 0;
        let mut guard = 0;
        while let Some(p) = parent[i] {
            i = p;
            d += 1;
            guard += 1;
            if guard > n {
                break;
            }
        }
        d
    };

    let mut out: Vec<Polygon> = Vec::new();
    let mut slot: Vec<Option<usize>> = vec![None; n];
    for i in 0..n {
        if depth(i) % 2 == 0 {
            let mut outer = rings[i].clone();
            if signed_area(&outer) < 0.0 {
                outer.reverse();
            }
            slot[i] = Some(out.len());
            out.push(Polygon { outer, holes: Vec::new() });
        }
    }
    for i in 0..n {
        if depth(i) % 2 == 1 {
            let Some(p) = parent[i] else { continue };
            let Some(k) = slot[p] else { continue };
            let mut ring = rings[i].clone();
            if signed_area(&ring) > 0.0 {
                ring.reverse();
            }
            out[k].holes.push(ring);
        }
    }
    out
}

pub fn signed_area(ring: &[[f64; 2]]) -> f64 {
    let n = ring.len();
    let mut a = 0.0;
    for i in 0..n {
        let p = ring[i];
        let q = ring[(i + 1) % n];
        a += p[0] * q[1] - q[0] * p[1];
    }
    a * 0.5
}

fn point_in_ring(p: [f64; 2], ring: &[[f64; 2]]) -> bool {
    let mut inside = false;
    let n = ring.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (ring[i], ring[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if p[0] < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

fn dedup(ring: &mut Ring) {
    ring.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9);
    if ring.len() > 1 {
        let (f, l) = (ring[0], ring[ring.len() - 1]);
        if (f[0] - l[0]).abs() < 1e-9 && (f[1] - l[1]).abs() < 1e-9 {
            ring.pop();
        }
    }
}

/// Collapses one-pixel spikes, where the traced border walks out along a
/// dead end and back over the same points. They carry no area and would leave
/// the cap triangulation short of the wall.
fn remove_spurs(ring: &mut Ring) {
    let eq = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9;
    for _ in 0..3 {
        let mut st: Vec<[f64; 2]> = Vec::with_capacity(ring.len());
        for &p in ring.iter() {
            if st.len() >= 2 && eq(st[st.len() - 2], p) {
                st.pop();
            } else {
                st.push(p);
            }
        }
        if st.len() == ring.len() {
            break;
        }
        st.rotate_left(1);
        *ring = st;
    }
}

fn chaikin(ring: &[[f64; 2]]) -> Ring {
    let n = ring.len();
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        let p = ring[i];
        let q = ring[(i + 1) % n];
        out.push([0.75 * p[0] + 0.25 * q[0], 0.75 * p[1] + 0.25 * q[1]]);
        out.push([0.25 * p[0] + 0.75 * q[0], 0.25 * p[1] + 0.75 * q[1]]);
    }
    out
}

fn rdp_closed(ring: &[[f64; 2]], eps: f64) -> Ring {
    if ring.len() < 4 || eps <= 0.0 {
        return ring.to_vec();
    }
    let p0 = ring[0];
    let far = ring
        .iter()
        .enumerate()
        .map(|(i, p)| (i, (p[0] - p0[0]).powi(2) + (p[1] - p0[1]).powi(2)))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0);
    if far == 0 {
        return ring.to_vec();
    }
    let mut a = rdp(&ring[..=far], eps);
    let mut second: Vec<[f64; 2]> = ring[far..].to_vec();
    second.push(ring[0]);
    let b = rdp(&second, eps);
    a.pop();
    a.extend_from_slice(&b[..b.len() - 1]);
    a
}

fn rdp(pts: &[[f64; 2]], eps: f64) -> Vec<[f64; 2]> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let (a, b) = (pts[0], pts[pts.len() - 1]);
    let mut max_d = 0.0;
    let mut idx = 0;
    for (i, p) in pts.iter().enumerate().skip(1).take(pts.len() - 2) {
        let d = point_seg_dist(*p, a, b);
        if d > max_d {
            max_d = d;
            idx = i;
        }
    }
    if max_d > eps {
        let mut left = rdp(&pts[..=idx], eps);
        let right = rdp(&pts[idx..], eps);
        left.pop();
        left.extend(right);
        left
    } else {
        vec![a, b]
    }
}

fn point_seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return ((p[0] - a[0]).powi(2) + (p[1] - a[1]).powi(2)).sqrt();
    }
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0);
    ((p[0] - (a[0] + t * dx)).powi(2) + (p[1] - (a[1] + t * dy)).powi(2)).sqrt()
}
