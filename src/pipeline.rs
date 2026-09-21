use crate::geometry::{assemble, intersect, rings, ContourOpts, Field, Polygon, Ring};
use crate::mesh::{cap, extrude, is_meshable, walls, write_stl, Tri};
use crate::params::Params;
use crate::segment::{segment, Segmentation};
use anyhow::{Context, Result};
use image::RgbaImage;
use std::path::{Path, PathBuf};

pub struct Build {
    pub seg: Segmentation,
    pub size_mm: [f32; 2],
    /// Base slab carrying the flange, z 0..flange_height.
    pub base: Vec<Polygon>,
    /// Lid of the base outside the blade, at flange_height.
    pub lid_outer: Vec<Polygon>,
    /// Lid of the base inside the blade, at flange_height.
    pub lid_inner: Vec<Polygon>,
    /// Cutting wall, z flange_height..flange_height+blade_height.
    pub blade: Vec<Polygon>,
    pub plate: Vec<Polygon>,
    /// Raised band tracing the plate edge, the embossed outline of the cookie.
    pub rim: Vec<Polygon>,
    pub detail: Vec<Polygon>,
    /// Shapes too tangled to triangulate, left out of the export.
    pub dropped: usize,
    /// How far curve smoothing had to be backed off to keep offsets apart.
    pub relaxed: usize,
}

pub fn build(img: &RgbaImage, p: &Params) -> Result<Build> {
    let seg = segment(img, p);
    let ppm = seg.ppm;
    let field = Field::new(&seg.silhouette);
    let dfield = Field::new(&seg.detail);

    // Smoothing followed by point reduction can nudge two offsets of the same
    // outline across each other on small or intricate shapes. The parts of one
    // solid share their rings, so a retry has to redraw all of them together.
    let mut relaxed = 0usize;
    let (mut base, mut lid_outer, mut lid_inner, mut blade, mut plate, mut rim, mut detail);
    loop {
        let opts = ContourOpts {
            ppm,
            mirror: p.mirror,
            smooth_iters: if relaxed >= 2 { 0 } else { p.smooth_iters },
            simplify_mm: if relaxed >= 1 { 0.0 } else { p.simplify_mm },
        };
        let level = |mm: f32| -> Vec<Ring> { rings(&field.below(mm, ppm), &opts) };

        base = Vec::new();
        lid_outer = Vec::new();
        lid_inner = Vec::new();
        blade = Vec::new();
        if p.cutter_enabled {
            // Four nested offsets of the silhouette. Every ring is traced once
            // and shared by the parts meeting along it, which is what keeps the
            // exported cutter a single closed surface.
            let lip = -p.inner_lip_width;
            let b0 = p.blade_offset;
            let b1 = b0 + p.blade_thickness;
            let out = b1 + p.flange_width;
            let (r_lip, r_b0, r_b1, r_out) = (level(lip), level(b0), level(b1), level(out));

            base = assemble(concat(&r_out, &r_lip));
            blade = assemble(concat(&r_b1, &r_b0));
            if p.flange_width > 1e-4 {
                lid_outer = assemble(concat(&r_out, &r_b1));
            }
            if b0 - lip > 1e-4 {
                lid_inner = assemble(concat(&r_b0, &r_lip));
            }
        }

        plate = Vec::new();
        rim = Vec::new();
        detail = Vec::new();
        if p.stamp_enabled {
            let edge = -p.plate_clearance;
            plate = assemble(level(edge));
            if p.rim_width > 1e-4 {
                rim = assemble(concat(&level(edge), &level(edge - p.rim_width)));
            }
            let grown = dfield.below(p.detail_expand, ppm);
            let inside = field.below(edge - p.rim_width - p.detail_inset, ppm);
            detail = assemble(rings(&intersect(&grown, &inside), &opts));
        }

        let shared_ok = [&base, &lid_outer, &lid_inner, &blade, &plate, &rim]
            .iter()
            .all(|set| set.iter().all(is_meshable));
        if shared_ok || relaxed >= 2 {
            break;
        }
        relaxed += 1;
    }

    // Detail islands are separate solids, so a hopeless one can just be left out.
    let mut dropped = 0;
    for set in [
        &mut base, &mut lid_outer, &mut lid_inner, &mut blade, &mut plate, &mut rim, &mut detail,
    ] {
        let before = set.len();
        set.retain(is_meshable);
        dropped += before - set.len();
    }

    Ok(Build {
        dropped,
        relaxed,
        size_mm: [seg.w as f32 / ppm, seg.h as f32 / ppm],
        seg,
        base,
        lid_outer,
        lid_inner,
        blade,
        plate,
        rim,
        detail,
    })
}

fn concat(a: &[Ring], b: &[Ring]) -> Vec<Ring> {
    let mut v = a.to_vec();
    v.extend_from_slice(b);
    v
}

pub fn cutter_tris(b: &Build, p: &Params) -> Result<Vec<Tri>> {
    let fh = p.flange_height;
    let top = fh + p.blade_height;
    let mut out = Vec::new();
    for poly in &b.base {
        cap(poly, 0.0, false, &mut out)?;
        walls(poly, 0.0, fh, &mut out);
    }
    for poly in b.lid_outer.iter().chain(b.lid_inner.iter()) {
        cap(poly, fh, true, &mut out)?;
    }
    for poly in &b.blade {
        walls(poly, fh, top, &mut out);
        cap(poly, top, true, &mut out)?;
    }
    Ok(out)
}

pub fn stamp_tris(b: &Build, p: &Params) -> Result<Vec<Tri>> {
    let mut out = Vec::new();
    for poly in &b.plate {
        extrude(poly, 0.0, p.plate_thickness, &mut out)?;
    }
    for poly in b.rim.iter().chain(b.detail.iter()) {
        // Sunk slightly into the plate so the two solids merge when sliced.
        extrude(poly, p.plate_thickness - 0.05, p.plate_thickness + p.detail_height, &mut out)?;
    }
    Ok(out)
}

/// Writes `<base>_cortador.stl` and `<base>_carimbo.stl`; returns the paths written.
pub fn export(b: &Build, p: &Params, base: &Path) -> Result<Vec<PathBuf>> {
    let stem = base
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("bolacha")
        .trim_end_matches("_cortador")
        .trim_end_matches("_carimbo")
        .to_string();
    let dir = base.parent().unwrap_or(Path::new("."));
    let mut written = Vec::new();
    let mut save = |suffix: &str, tris: Vec<Tri>| -> Result<()> {
        if tris.is_empty() {
            return Ok(());
        }
        let path = dir.join(format!("{stem}_{suffix}.stl"));
        let f = std::fs::File::create(&path).with_context(|| format!("create {}", path.display()))?;
        write_stl(std::io::BufWriter::new(f), &tris)?;
        written.push(path);
        Ok(())
    };
    if p.cutter_enabled {
        save("cortador", cutter_tris(b, p)?)?;
    }
    if p.stamp_enabled {
        save("carimbo", stamp_tris(b, p)?)?;
    }
    Ok(written)
}

pub fn load_image(path: &Path) -> Result<RgbaImage> {
    Ok(image::open(path)
        .with_context(|| format!("open {}", path.display()))?
        .to_rgba8())
}
