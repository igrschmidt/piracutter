use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SizeMode {
    Width,
    Height,
    Longest,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum BgMode {
    Auto,
    Alpha,
    BorderColor,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    // sizing
    pub size_mm: f32,
    pub size_mode: SizeMode,
    pub px_per_mm: f32,
    /// Printed parts are flipped over when pressed into dough, so the
    /// geometry must be a mirror of the picture for the cookie to read right.
    pub mirror: bool,

    // segmentation
    pub bg_mode: BgMode,
    pub bg_tolerance: f32,
    pub keep_holes: bool,
    pub detail_threshold: u8,
    pub min_blob_mm2: f32,
    pub min_detail_mm2: f32,

    // cutter
    pub cutter_enabled: bool,
    pub blade_offset: f32,
    pub blade_thickness: f32,
    pub blade_height: f32,
    pub flange_width: f32,
    pub flange_height: f32,
    pub inner_lip_width: f32,

    // stamp
    pub stamp_enabled: bool,
    pub plate_thickness: f32,
    pub plate_clearance: f32,
    pub detail_height: f32,
    pub detail_expand: f32,
    pub detail_inset: f32,
    pub rim_width: f32,

    // contour cleanup
    pub smooth_mm: f32,
    pub simplify_mm: f32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            size_mm: 80.0,
            size_mode: SizeMode::Longest,
            px_per_mm: 12.0,
            mirror: true,

            bg_mode: BgMode::Auto,
            bg_tolerance: 0.12,
            keep_holes: false,
            detail_threshold: 100,
            min_blob_mm2: 1.0,
            min_detail_mm2: 0.15,

            cutter_enabled: true,
            blade_offset: 0.0,
            blade_thickness: 0.8,
            blade_height: 15.0,
            flange_width: 4.0,
            flange_height: 1.5,
            inner_lip_width: 0.0,

            stamp_enabled: true,
            plate_thickness: 3.0,
            plate_clearance: 1.2,
            detail_height: 1.5,
            detail_expand: 0.15,
            detail_inset: 0.3,
            rim_width: 0.8,

            smooth_mm: 0.25,
            simplify_mm: 0.01,
        }
    }
}

impl Params {
    pub fn margin_mm(&self) -> f32 {
        self.blade_offset + self.blade_thickness + self.flange_width + self.inner_lip_width + 2.0
    }
}
