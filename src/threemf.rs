//! 3MF writer. A 3MF file is a zip holding an XML model, which lets both
//! parts travel as named objects in one file instead of two loose meshes.

use crate::mesh::Tri;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{Seek, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/>
</Types>"#;

const RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rel0" Target="/3D/3dmodel.model" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodelrelationship"/>
</Relationships>"#;

/// One named solid in the exported file.
pub struct Object<'a> {
    pub name: &'a str,
    pub tris: &'a [Tri],
}

pub fn write<W: Write + Seek>(sink: W, objects: &[Object<'_>]) -> Result<()> {
    let mut zip = ZipWriter::new(sink);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    zip.start_file("[Content_Types].xml", opts)?;
    zip.write_all(CONTENT_TYPES.as_bytes())?;
    zip.start_file("_rels/.rels", opts)?;
    zip.write_all(RELS.as_bytes())?;
    zip.start_file("3D/3dmodel.model", opts)?;
    zip.write_all(model_xml(objects)?.as_bytes())?;
    zip.finish().context("closing the 3MF container")?;
    Ok(())
}

fn model_xml(objects: &[Object<'_>]) -> Result<String> {
    let mut out = String::with_capacity(1 << 20);
    out.push_str(concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<model unit=\"millimeter\" xml:lang=\"en-US\" ",
        "xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n",
        " <metadata name=\"Application\">PiraCutter</metadata>\n",
        " <resources>\n"
    ));

    for (i, obj) in objects.iter().enumerate() {
        let (verts, faces) = weld(obj.tris);
        write!(
            out,
            "  <object id=\"{}\" type=\"model\" name=\"{}\">\n   <mesh>\n    <vertices>\n",
            i + 1,
            escape(obj.name)
        )?;
        for v in &verts {
            writeln!(
                out,
                "     <vertex x=\"{}\" y=\"{}\" z=\"{}\"/>",
                trim(v[0]),
                trim(v[1]),
                trim(v[2])
            )?;
        }
        out.push_str("    </vertices>\n    <triangles>\n");
        for f in &faces {
            writeln!(
                out,
                "     <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>",
                f[0], f[1], f[2]
            )?;
        }
        out.push_str("    </triangles>\n   </mesh>\n  </object>\n");
    }

    out.push_str(" </resources>\n <build>\n");
    for i in 0..objects.len() {
        writeln!(out, "  <item objectid=\"{}\"/>", i + 1)?;
    }
    out.push_str(" </build>\n</model>\n");
    Ok(out)
}

/// Collapses shared corners into an index list. The mesh is built from shared
/// contour points, so identical corners are bit-identical and match exactly.
fn weld(tris: &[Tri]) -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let mut seen: HashMap<[u32; 3], u32> = HashMap::with_capacity(tris.len());
    let mut verts: Vec<[f32; 3]> = Vec::with_capacity(tris.len());
    let mut faces = Vec::with_capacity(tris.len());
    for t in tris {
        let mut idx = [0u32; 3];
        for (k, v) in t.iter().enumerate() {
            let key = [v[0].to_bits(), v[1].to_bits(), v[2].to_bits()];
            idx[k] = *seen.entry(key).or_insert_with(|| {
                verts.push(*v);
                (verts.len() - 1) as u32
            });
        }
        if idx[0] != idx[1] && idx[1] != idx[2] && idx[0] != idx[2] {
            faces.push(idx);
        }
    }
    (verts, faces)
}

fn trim(v: f32) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" {
        "0".into()
    } else {
        s.into()
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
