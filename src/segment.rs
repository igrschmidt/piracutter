use crate::params::{BgMode, Params, SizeMode};
use image::imageops::FilterType;
use image::{GrayImage, Luma, RgbaImage};
use imageproc::region_labelling::{connected_components, Connectivity};
use std::collections::VecDeque;

pub struct Segmentation {
    pub w: u32,
    pub h: u32,
    pub ppm: f32,
    /// Everything that is not background: body fill plus dark detail lines.
    pub silhouette: GrayImage,
    /// Dark lines and spots that become the raised part of the stamp.
    pub detail: GrayImage,
}

pub fn segment(src: &RgbaImage, p: &Params) -> Segmentation {
    let src = crop_to_subject(src, p);
    let ppm = p.px_per_mm;
    let target = p.size_mm * ppm;
    let (sw, sh) = (src.width() as f32, src.height() as f32);
    let scale = match p.size_mode {
        SizeMode::Width => target / sw,
        SizeMode::Height => target / sh,
        SizeMode::Longest => target / sw.max(sh),
    };
    let tw = (sw * scale).round().max(8.0) as u32;
    let th = (sh * scale).round().max(8.0) as u32;
    let img = image::imageops::resize(&src, tw, th, FilterType::Triangle);

    let bg = background(&img, p);
    let m = (p.margin_mm() * ppm).ceil() as u32;
    let w = tw + 2 * m;
    let h = th + 2 * m;
    let mut silhouette = GrayImage::new(w, h);
    let mut detail = GrayImage::new(w, h);
    for y in 0..th {
        for x in 0..tw {
            if bg[(y * tw + x) as usize] {
                continue;
            }
            let px = img.get_pixel(x, y);
            silhouette.put_pixel(x + m, y + m, Luma([255]));
            let lum = 0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32;
            if lum < p.detail_threshold as f32 {
                detail.put_pixel(x + m, y + m, Luma([255]));
            }
        }
    }

    remove_small_blobs(&mut silhouette, (p.min_blob_mm2 * ppm * ppm) as usize);
    remove_small_blobs(&mut detail, (p.min_detail_mm2 * ppm * ppm) as usize);

    Segmentation { w, h, ppm, silhouette, detail }
}

/// Sizing is meant to describe the cookie, so padding around the artwork must
/// not count towards it.
fn crop_to_subject(src: &RgbaImage, p: &Params) -> RgbaImage {
    let (sw, sh) = src.dimensions();
    let probe_scale = 768.0 / sw.max(sh) as f32;
    let probe = if probe_scale < 1.0 {
        image::imageops::resize(
            src,
            (sw as f32 * probe_scale).round().max(4.0) as u32,
            (sh as f32 * probe_scale).round().max(4.0) as u32,
            FilterType::Triangle,
        )
    } else {
        src.clone()
    };
    let (pw, ph) = probe.dimensions();
    let bg = background(&probe, p);

    let (mut x0, mut y0, mut x1, mut y1) = (pw, ph, 0u32, 0u32);
    for y in 0..ph {
        for x in 0..pw {
            if !bg[(y * pw + x) as usize] {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if x1 < x0 || y1 < y0 {
        return src.clone();
    }

    let fx = sw as f32 / pw as f32;
    let fy = sh as f32 / ph as f32;
    let pad = 2.0;
    let cx0 = ((x0 as f32 * fx) - pad).max(0.0) as u32;
    let cy0 = ((y0 as f32 * fy) - pad).max(0.0) as u32;
    let cx1 = (((x1 + 1) as f32 * fx) + pad).min(sw as f32) as u32;
    let cy1 = (((y1 + 1) as f32 * fy) + pad).min(sh as f32) as u32;
    image::imageops::crop_imm(src, cx0, cy0, (cx1 - cx0).max(1), (cy1 - cy0).max(1)).to_image()
}

/// Per-pixel background flags for `img`.
fn background(img: &RgbaImage, p: &Params) -> Vec<bool> {
    let (w, h) = img.dimensions();
    let has_alpha = img.pixels().any(|px| px[3] < 128);
    let use_alpha = match p.bg_mode {
        BgMode::Alpha => true,
        BgMode::BorderColor => false,
        BgMode::Auto => has_alpha,
    };

    let mut candidate = vec![false; (w * h) as usize];
    if use_alpha {
        for (i, px) in img.pixels().enumerate() {
            candidate[i] = px[3] < 128;
        }
    } else {
        let bg = border_color(img);
        let tol = p.bg_tolerance * 441.67;
        for (i, px) in img.pixels().enumerate() {
            let d = ((px[0] as f32 - bg[0]).powi(2)
                + (px[1] as f32 - bg[1]).powi(2)
                + (px[2] as f32 - bg[2]).powi(2))
            .sqrt();
            candidate[i] = d < tol;
        }
    }

    if p.keep_holes {
        candidate
    } else {
        flood_from_border(&candidate, w, h)
    }
}

fn border_color(img: &RgbaImage) -> [f32; 3] {
    let (w, h) = img.dimensions();
    let mut sum = [0f64; 3];
    let mut count = 0f64;
    let mut add = |x: u32, y: u32| {
        let px = img.get_pixel(x, y);
        for c in 0..3 {
            sum[c] += px[c] as f64;
        }
        count += 1.0;
    };
    for x in 0..w {
        add(x, 0);
        add(x, h - 1);
    }
    for y in 1..h.saturating_sub(1) {
        add(0, y);
        add(w - 1, y);
    }
    [
        (sum[0] / count) as f32,
        (sum[1] / count) as f32,
        (sum[2] / count) as f32,
    ]
}

fn flood_from_border(candidate: &[bool], w: u32, h: u32) -> Vec<bool> {
    let mut out = vec![false; candidate.len()];
    let mut q = VecDeque::new();
    let idx = |x: u32, y: u32| (y * w + x) as usize;
    let seed = |x: u32, y: u32, q: &mut VecDeque<(u32, u32)>, out: &mut Vec<bool>| {
        let i = idx(x, y);
        if candidate[i] && !out[i] {
            out[i] = true;
            q.push_back((x, y));
        }
    };
    for x in 0..w {
        seed(x, 0, &mut q, &mut out);
        seed(x, h - 1, &mut q, &mut out);
    }
    for y in 0..h {
        seed(0, y, &mut q, &mut out);
        seed(w - 1, y, &mut q, &mut out);
    }
    while let Some((x, y)) = q.pop_front() {
        for (nx, ny) in [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ] {
            if nx < w && ny < h {
                let i = idx(nx, ny);
                if candidate[i] && !out[i] {
                    out[i] = true;
                    q.push_back((nx, ny));
                }
            }
        }
    }
    out
}

fn remove_small_blobs(mask: &mut GrayImage, min_px: usize) {
    if min_px <= 1 {
        return;
    }
    let labels = connected_components(mask, Connectivity::Eight, Luma([0u8]));
    let mut sizes: Vec<usize> = Vec::new();
    for l in labels.pixels() {
        let l = l[0] as usize;
        if l >= sizes.len() {
            sizes.resize(l + 1, 0);
        }
        sizes[l] += 1;
    }
    for (x, y, l) in labels.enumerate_pixels() {
        let l = l[0] as usize;
        if l != 0 && sizes[l] < min_px {
            mask.put_pixel(x, y, Luma([0]));
        }
    }
}
