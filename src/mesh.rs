use crate::geometry::{signed_area, Polygon};
use anyhow::{anyhow, Result};
use std::collections::HashSet;
use std::io::Write;

pub type Tri = [[f32; 3]; 3];

/// Triangulated lid at height `z`. `up` faces +Z, otherwise -Z.
pub fn cap(poly: &Polygon, z: f32, up: bool, out: &mut Vec<Tri>) -> Result<()> {
    let mut flat: Vec<f64> = Vec::new();
    let mut hole_idx: Vec<usize> = Vec::new();
    for p in &poly.outer {
        flat.extend_from_slice(p);
    }
    for h in &poly.holes {
        hole_idx.push(flat.len() / 2);
        for p in h {
            flat.extend_from_slice(p);
        }
    }
    let tris = earcutr::earcut(&flat, &hole_idx, 2).map_err(|e| anyhow!("triangulation: {e:?}"))?;
    let pt = |i: usize| [flat[2 * i] as f32, flat[2 * i + 1] as f32, z];
    for t in tris.chunks_exact(3) {
        let (a, b, c) = (t[0], t[1], t[2]);
        let ring = [
            [flat[2 * a], flat[2 * a + 1]],
            [flat[2 * b], flat[2 * b + 1]],
            [flat[2 * c], flat[2 * c + 1]],
        ];
        let ccw = signed_area(&ring) > 0.0;
        let (a, b, c) = if ccw == up { (a, b, c) } else { (a, c, b) };
        out.push([pt(a), pt(b), pt(c)]);
    }
    Ok(())
}

/// Vertical skirt along every ring of `poly`, from `z0` up to `z1`.
pub fn walls(poly: &Polygon, z0: f32, z1: f32, out: &mut Vec<Tri>) {
    for ring in std::iter::once(&poly.outer).chain(poly.holes.iter()) {
        let n = ring.len();
        for i in 0..n {
            let p = ring[i];
            let q = ring[(i + 1) % n];
            let p0 = [p[0] as f32, p[1] as f32, z0];
            let p1 = [q[0] as f32, q[1] as f32, z0];
            let q0 = [p[0] as f32, p[1] as f32, z1];
            let q1 = [q[0] as f32, q[1] as f32, z1];
            out.push([p0, p1, q1]);
            out.push([p0, q1, q0]);
        }
    }
}

/// True when the triangulated cap is bounded by exactly the polygon's rings.
/// Anything else means ear clipping skipped part of a self-touching contour,
/// which would leave the cap and the wall disagreeing about the border.
pub fn is_meshable(poly: &Polygon) -> bool {
    let mut flat: Vec<f64> = Vec::new();
    let mut hole_idx: Vec<usize> = Vec::new();
    let mut ring_edges: Vec<(usize, usize)> = Vec::new();
    let push_ring = |ring: &[[f64; 2]], flat: &mut Vec<f64>, edges: &mut Vec<(usize, usize)>| {
        let start = flat.len() / 2;
        for p in ring {
            flat.extend_from_slice(p);
        }
        let n = ring.len();
        for i in 0..n {
            edges.push((start + i, start + (i + 1) % n));
        }
    };
    push_ring(&poly.outer, &mut flat, &mut ring_edges);
    for h in &poly.holes {
        hole_idx.push(flat.len() / 2);
        push_ring(h, &mut flat, &mut ring_edges);
    }

    let Ok(tris) = earcutr::earcut(&flat, &hole_idx, 2) else {
        return false;
    };
    let mut seen: HashSet<(usize, usize)> = HashSet::with_capacity(tris.len());
    for t in tris.chunks_exact(3) {
        let ring = [
            [flat[2 * t[0]], flat[2 * t[0] + 1]],
            [flat[2 * t[1]], flat[2 * t[1] + 1]],
            [flat[2 * t[2]], flat[2 * t[2] + 1]],
        ];
        let t = if signed_area(&ring) > 0.0 {
            [t[0], t[1], t[2]]
        } else {
            [t[0], t[2], t[1]]
        };
        for i in 0..3 {
            if !seen.insert((t[i], t[(i + 1) % 3])) {
                return false;
            }
        }
    }
    ring_edges
        .iter()
        .all(|e| seen.contains(e) && !seen.contains(&(e.1, e.0)))
}

pub fn extrude(poly: &Polygon, z0: f32, z1: f32, out: &mut Vec<Tri>) -> Result<()> {
    cap(poly, z1, true, out)?;
    cap(poly, z0, false, out)?;
    walls(poly, z0, z1, out);
    Ok(())
}

pub fn write_stl<W: Write>(mut w: W, tris: &[Tri]) -> Result<()> {
    let mut header = [0u8; 80];
    let tag = b"cookiecut";
    header[..tag.len()].copy_from_slice(tag);
    w.write_all(&header)?;
    w.write_all(&(tris.len() as u32).to_le_bytes())?;
    for t in tris {
        let n = normal(t);
        for v in n.iter().chain(t.iter().flatten()) {
            w.write_all(&v.to_le_bytes())?;
        }
        w.write_all(&0u16.to_le_bytes())?;
    }
    Ok(())
}

fn normal(t: &Tri) -> [f32; 3] {
    let u = [t[1][0] - t[0][0], t[1][1] - t[0][1], t[1][2] - t[0][2]];
    let v = [t[2][0] - t[0][0], t[2][1] - t[0][1], t[2][2] - t[0][2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len == 0.0 {
        [0.0, 0.0, 1.0]
    } else {
        [n[0] / len, n[1] / len, n[2] / len]
    }
}
