use cookiecut::mesh::Tri;
use cookiecut::params::{Params, SizeMode};
use cookiecut::pipeline::{build, cutter_tris, stamp_tris};
use image::{Rgba, RgbaImage};
use std::collections::HashMap;

include!("support/sample.rs");

/// Counts directed edges; a closed surface uses each one exactly once and its
/// reverse exactly once.
fn open_edges(tris: &[Tri]) -> usize {
    let key = |v: [f32; 3]| {
        (
            (v[0] * 4096.0).round() as i64,
            (v[1] * 4096.0).round() as i64,
            (v[2] * 4096.0).round() as i64,
        )
    };
    let mut count: HashMap<((i64, i64, i64), (i64, i64, i64)), i32> = HashMap::new();
    for t in tris {
        for i in 0..3 {
            *count.entry((key(t[i]), key(t[(i + 1) % 3]))).or_insert(0) += 1;
        }
    }
    count
        .iter()
        .filter(|(&(a, b), &n)| n != 1 || count.get(&(b, a)).copied().unwrap_or(0) != 1)
        .count()
}

fn volume(tris: &[Tri]) -> f64 {
    tris.iter()
        .map(|t| {
            let [a, b, c] = [t[0], t[1], t[2]].map(|v| v.map(|x| x as f64));
            (a[0] * (b[1] * c[2] - c[1] * b[2]) - a[1] * (b[0] * c[2] - c[0] * b[2])
                + a[2] * (b[0] * c[1] - c[0] * b[1]))
                / 6.0
        })
        .sum()
}

fn variants() -> Vec<(&'static str, Params)> {
    let d = Params::default();
    let mut v = vec![("default", d.clone())];
    let mut add = |name, f: &dyn Fn(&mut Params)| {
        let mut p = d.clone();
        f(&mut p);
        v.push((name, p));
    };
    add("no_flange", &|p| p.flange_width = 0.0);
    add("inner_lip", &|p| p.inner_lip_width = 2.0);
    add("blade_offset", &|p| p.blade_offset = 1.0);
    add("offset_and_lip", &|p| {
        p.blade_offset = 1.0;
        p.inner_lip_width = 1.5;
    });
    add("keep_holes", &|p| p.keep_holes = true);
    add("no_mirror", &|p| p.mirror = false);
    add("no_smoothing", &|p| {
        p.smooth_iters = 0;
        p.simplify_mm = 0.0;
    });
    add("high_res", &|p| p.px_per_mm = 16.0);
    add("low_res", &|p| {
        p.px_per_mm = 4.0;
        p.min_blob_mm2 = 2.0;
    });
    add("small", &|p| p.size_mm = 25.0);
    add("smaller", &|p| p.size_mm = 18.0);
    add("large", &|p| p.size_mm = 150.0);
    add("thick_detail", &|p| p.detail_expand = 0.8);
    add("thin_detail", &|p| p.detail_expand = -0.2);
    add("cutter_only", &|p| p.stamp_enabled = false);
    add("stamp_only", &|p| p.cutter_enabled = false);
    v
}

#[test]
fn exports_are_closed_surfaces() {
    let img = sample_image();
    for (name, p) in variants() {
        let b = build(&img, &p).unwrap_or_else(|e| panic!("{name}: build failed: {e}"));
        if p.cutter_enabled {
            let t = cutter_tris(&b, &p).unwrap();
            assert!(!t.is_empty(), "{name}: cutter empty");
            assert_eq!(open_edges(&t), 0, "{name}: cutter is not closed");
            assert!(volume(&t) > 0.0, "{name}: cutter volume {}", volume(&t));
        }
        if p.stamp_enabled {
            let t = stamp_tris(&b, &p).unwrap();
            assert!(!t.is_empty(), "{name}: stamp empty");
            assert_eq!(open_edges(&t), 0, "{name}: stamp is not closed");
            assert!(volume(&t) > 0.0, "{name}: stamp volume {}", volume(&t));
        }
    }
}

#[test]
fn default_settings_drop_nothing() {
    let b = build(&sample_image(), &Params::default()).unwrap();
    assert_eq!(b.dropped, 0);
    assert!(b.detail.len() > 10, "expected detail shapes, got {}", b.detail.len());
}

#[test]
fn size_setting_measures_the_artwork_not_the_canvas() {
    // Same drawing, one with a wide empty border: both must print the same size.
    let tight = sample_image();
    let mut padded = image::RgbaImage::from_pixel(900, 900, image::Rgba([210, 210, 210, 255]));
    image::imageops::overlay(&mut padded, &tight, 200, 200);

    for size in [30.0f32, 80.0, 150.0] {
        let mut spans = Vec::new();
        for img in [&tight, &padded] {
            let p = Params { size_mm: size, size_mode: SizeMode::Longest, ..Default::default() };
            let b = build(img, &p).unwrap();
            let pts: Vec<[f64; 2]> =
                b.blade.iter().flat_map(|q| q.outer.iter().copied()).collect();
            let span = |i: usize| {
                pts.iter().map(|v| v[i]).fold(f64::MIN, f64::max)
                    - pts.iter().map(|v| v[i]).fold(f64::MAX, f64::min)
            };
            spans.push(span(0).max(span(1)));
        }
        let expected = (size + 2.0 * (0.0 + 0.8)) as f64;
        for (i, got) in spans.iter().enumerate() {
            assert!(
                (got - expected).abs() < expected * 0.06,
                "size {size} ({}): longest side {got:.1} mm, expected about {expected:.1} mm",
                ["tight", "padded"][i]
            );
        }
    }
}
