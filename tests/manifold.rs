use piracutter::mesh::Tri;
use piracutter::params::{Params, SizeMode};
use piracutter::pipeline::{build, cutter_tris, stamp_tris, Format};
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
        p.smooth_mm = 0.0;
        p.simplify_mm = 0.0;
    });
    add("heavy_smoothing", &|p| p.smooth_mm = 0.8);
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

/// Smoothing the distance field must not round a real corner away.
#[test]
fn sharp_features_survive_smoothing() {
    let img = sample_image();
    let span = |p: &Params| {
        let b = build(&img, p).unwrap();
        let ys: Vec<f64> = b.blade.iter().flat_map(|q| q.outer.iter().map(|v| v[1])).collect();
        ys.iter().cloned().fold(f64::MIN, f64::max) - ys.iter().cloned().fold(f64::MAX, f64::min)
    };
    let sharp = span(&Params { smooth_mm: 0.0, ..Default::default() });
    let smoothed = span(&Params::default());
    assert!(
        (sharp - smoothed).abs() < 1.0,
        "default smoothing moved the ear tips by {:.2} mm",
        (sharp - smoothed).abs()
    );
}

/// Winding used to be re-derived per triangle, which read noise off collinear
/// slivers and silently emptied the export at most resolutions.
#[test]
fn every_resolution_produces_a_model() {
    let img = sample_image();
    for ppm in [4.0f32, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 16.0, 18.0, 20.0] {
        let p = Params { px_per_mm: ppm, ..Default::default() };
        let b = build(&img, &p).unwrap();
        assert_eq!(b.blade.len(), 1, "{ppm} px/mm: blade missing");
        assert_eq!(b.plate.len(), 1, "{ppm} px/mm: plate missing");
        assert!(b.detail.len() > 8, "{ppm} px/mm: only {} detail shapes", b.detail.len());
        for (part, tris) in [
            ("cutter", cutter_tris(&b, &p).unwrap()),
            ("stamp", stamp_tris(&b, &p).unwrap()),
        ] {
            assert_eq!(open_edges(&tris), 0, "{ppm} px/mm: {part} is not closed");
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

/// The 3MF writer welds shared corners, so the file has to be read back to
/// confirm the indexed mesh is still a closed surface.
#[test]
fn three_mf_holds_both_parts_as_closed_meshes() {
    let p = Params::default();
    let b = build(&sample_image(), &p).unwrap();
    let dir = std::env::temp_dir().join("piracutter-test-3mf");
    std::fs::create_dir_all(&dir).unwrap();
    let written =
        piracutter::pipeline::export(&b, &p, &dir.join("peca.3mf"), Format::ThreeMf).unwrap();
    assert_eq!(written.len(), 1, "3MF should be a single file");

    let file = std::fs::File::open(&written[0]).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    for required in ["[Content_Types].xml", "_rels/.rels", "3D/3dmodel.model"] {
        assert!(zip.by_name(required).is_ok(), "missing {required}");
    }
    let xml = {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("3D/3dmodel.model").unwrap(), &mut s)
            .unwrap();
        s
    };

    let objects: Vec<&str> = xml.split("<object ").skip(1).collect();
    assert_eq!(objects.len(), 2, "expected a cutter and a stamp");
    assert!(xml.contains("name=\"Cortador\""));
    assert!(xml.contains("name=\"Carimbo\""));

    for obj in objects {
        let count = |tag: &str| obj.matches(tag).count();
        let verts = count("<vertex ");
        let faces = count("<triangle ");
        assert!(verts > 100 && faces > 100, "object looks empty");
        // Welding should leave roughly two faces per corner, not one per face.
        assert!(faces < 3 * verts, "vertices were not welded: {verts} for {faces} faces");

        let mut edges: HashMap<(u32, u32), i32> = HashMap::new();
        for t in obj.split("<triangle ").skip(1) {
            let idx: Vec<u32> = ["v1=\"", "v2=\"", "v3=\""]
                .iter()
                .map(|k| {
                    let rest = &t[t.find(k).unwrap() + k.len()..];
                    rest[..rest.find('"').unwrap()].parse().unwrap()
                })
                .collect();
            for i in 0..3 {
                *edges.entry((idx[i], idx[(i + 1) % 3])).or_insert(0) += 1;
            }
        }
        let open = edges
            .iter()
            .filter(|(&(a, b), &n)| n != 1 || edges.get(&(b, a)).copied().unwrap_or(0) != 1)
            .count();
        assert_eq!(open, 0, "welded mesh has {open} unmatched edges");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
