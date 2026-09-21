use cookiecut::geometry::Polygon;
use cookiecut::params::{BgMode, Params, SizeMode};
use cookiecut::pipeline::{build, export, load_image, Build};
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use std::path::PathBuf;

pub struct App {
    params: Params,
    image: Option<image::RgbaImage>,
    image_path: Option<PathBuf>,
    result: Option<Build>,
    seg_tex: Option<egui::TextureHandle>,
    status: String,
    dirty: bool,
    show_seg: bool,
    show_blade: bool,
    show_flange: bool,
    show_plate: bool,
    show_detail: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, initial: Option<PathBuf>) -> Self {
        let params = cc
            .storage
            .and_then(|s| eframe::get_value::<Params>(s, "params"))
            .unwrap_or_default();
        let mut app = Self {
            params,
            image: None,
            image_path: None,
            result: None,
            seg_tex: None,
            status: "Open an image (File > Open, or drop it here).".into(),
            dirty: false,
            show_seg: true,
            show_blade: true,
            show_flange: true,
            show_plate: true,
            show_detail: true,
        };
        if let Some(p) = initial {
            app.open(p);
        }
        app
    }

    fn open(&mut self, path: PathBuf) {
        match load_image(&path) {
            Ok(img) => {
                self.status = format!("Loaded {} ({}x{})", path.display(), img.width(), img.height());
                self.image = Some(img);
                self.image_path = Some(path);
                self.dirty = true;
            }
            Err(e) => self.status = format!("Error: {e:#}"),
        }
    }

    fn rebuild(&mut self, ctx: &egui::Context) {
        self.dirty = false;
        let Some(img) = &self.image else { return };
        let t = std::time::Instant::now();
        match build(img, &self.params) {
            Ok(b) => {
                self.seg_tex = Some(ctx.load_texture(
                    "seg",
                    seg_image(&b),
                    egui::TextureOptions::NEAREST,
                ));
                self.status = format!(
                    "Built in {} ms. blade {} base {} plate {} detail {} polygons.",
                    t.elapsed().as_millis(),
                    b.blade.len(),
                    b.base.len(),
                    b.plate.len(),
                    b.detail.len()
                ) + &match (b.relaxed, b.dropped) {
                    (0, 0) => String::new(),
                    (r, 0) => format!(" Smoothing eased off ({r}) to keep offsets apart."),
                    (0, d) => format!(" {d} shape(s) too tangled to mesh, left out."),
                    (r, d) => format!(
                        " Smoothing eased off ({r}); {d} shape(s) too tangled to mesh, left out."
                    ),
                };
                self.result = Some(b);
            }
            Err(e) => self.status = format!("Error: {e:#}"),
        }
    }

    fn export_dialog(&mut self) {
        let Some(b) = &self.result else {
            self.status = "Nothing to export.".into();
            return;
        };
        let suggested = self
            .image_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("cookie")
            .to_string();
        let mut dlg = rfd::FileDialog::new()
            .set_file_name(format!("{suggested}.stl"))
            .add_filter("STL", &["stl"]);
        if let Some(dir) = self.image_path.as_ref().and_then(|p| p.parent()) {
            dlg = dlg.set_directory(dir);
        }
        if let Some(path) = dlg.save_file() {
            match export(b, &self.params, &path) {
                Ok(paths) => {
                    let names: Vec<String> = paths
                        .iter()
                        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                        .collect();
                    self.status = format!("Wrote {}", names.join(", "));
                }
                Err(e) => self.status = format!("Export failed: {e:#}"),
            }
        }
    }

    fn params_ui(&mut self, ui: &mut egui::Ui) {
        let p = &mut self.params;
        let mut changed = false;
        macro_rules! s {
            ($ui:expr, $field:expr, $range:expr, $label:expr, $suffix:expr, $hint:expr) => {
                changed |= $ui
                    .add(egui::Slider::new(&mut $field, $range).text($label).suffix($suffix))
                    .on_hover_text($hint)
                    .changed();
            };
        }

        ui.heading("Size");
        s!(ui, p.size_mm, 15.0..=250.0, "Cookie size", " mm", "Measured across the artwork itself. Blank space around the picture is ignored, and the flange adds to this.");
        egui::ComboBox::from_label("Measured across")
            .selected_text(match p.size_mode {
                SizeMode::Width => "width",
                SizeMode::Height => "height",
                SizeMode::Longest => "longest side",
            })
            .show_ui(ui, |ui| {
                for (m, label) in [
                    (SizeMode::Longest, "longest side"),
                    (SizeMode::Width, "width"),
                    (SizeMode::Height, "height"),
                ] {
                    changed |= ui.selectable_value(&mut p.size_mode, m, label).changed();
                }
            });
        s!(ui, p.px_per_mm, 3.0..=20.0, "Resolution", " px/mm", "Higher = smoother curves, slower. 8 is plenty for printing.");
        changed |= ui
            .checkbox(&mut p.mirror, "Mirror geometry")
            .on_hover_text("Keep on. Parts are flipped when pressed into dough, so the model must be a mirror image.")
            .changed();

        ui.separator();
        ui.heading("Image");
        egui::ComboBox::from_label("Background")
            .selected_text(format!("{:?}", p.bg_mode))
            .show_ui(ui, |ui| {
                for m in [BgMode::Auto, BgMode::Alpha, BgMode::BorderColor] {
                    let label = format!("{m:?}");
                    changed |= ui.selectable_value(&mut p.bg_mode, m, label).changed();
                }
            });
        s!(ui, p.bg_tolerance, 0.01..=0.6, "Background tolerance", "", "How far a colour may differ from the image border colour and still count as background.");
        changed |= ui
            .checkbox(&mut p.keep_holes, "Keep enclosed holes")
            .on_hover_text("Off: background trapped inside the shape becomes part of the cookie. On: it becomes a hole with its own blade.")
            .changed();
        s!(ui, p.detail_threshold, 0..=255, "Detail darkness", "", "Pixels darker than this become raised stamp lines.");
        s!(ui, p.min_blob_mm2, 0.0..=20.0, "Min shape area", " mm²", "Drops silhouette specks smaller than this.");
        s!(ui, p.min_detail_mm2, 0.0..=5.0, "Min detail area", " mm²", "Drops detail specks smaller than this.");

        ui.separator();
        changed |= ui.checkbox(&mut p.cutter_enabled, "Cutter").changed();
        ui.add_enabled_ui(p.cutter_enabled, |ui| {
            s!(ui, p.blade_thickness, 0.4..=3.0, "Blade thickness", " mm", "Two nozzle widths (0.8) prints as a clean two-wall blade.");
            s!(ui, p.blade_height, 5.0..=40.0, "Blade height", " mm", "");
            s!(ui, p.blade_offset, 0.0..=3.0, "Blade offset", " mm", "Gap between the silhouette edge and the blade inner face.");
            s!(ui, p.flange_width, 0.0..=15.0, "Flange width", " mm", "Outward lip at the base you press on.");
            s!(ui, p.flange_height, 0.4..=6.0, "Flange height", " mm", "");
            s!(ui, p.inner_lip_width, 0.0..=5.0, "Inner lip width", " mm", "Optional lip inside the blade, same height as flange. Adds stiffness on thin shapes.");
        });

        ui.separator();
        changed |= ui.checkbox(&mut p.stamp_enabled, "Stamp").changed();
        ui.add_enabled_ui(p.stamp_enabled, |ui| {
            s!(ui, p.plate_thickness, 1.0..=10.0, "Plate thickness", " mm", "");
            s!(ui, p.plate_clearance, 0.0..=4.0, "Plate clearance", " mm", "How much smaller than the cutter the plate is, so it fits inside the blade.");
            s!(ui, p.detail_height, 0.4..=5.0, "Detail height", " mm", "How far the lines stand out from the plate.");
            s!(ui, p.detail_expand, -0.5..=1.5, "Detail thicken", " mm", "Grow (or shrink, negative) the detail lines. Lines under ~0.8 mm total will not print.");
            s!(ui, p.rim_width, 0.0..=4.0, "Outline rim", " mm", "Raised band tracing the plate edge, so the cookie gets an embossed outline. Zero turns it off.");
            s!(ui, p.detail_inset, 0.0..=3.0, "Detail edge inset", " mm", "Keep the rest of the detail this far inside the rim.");
        });

        ui.separator();
        ui.heading("Curves");
        s!(ui, p.smooth_iters, 0..=4, "Smoothing", "", "Chaikin passes over traced contours.");
        s!(ui, p.simplify_mm, 0.0..=0.3, "Simplify", " mm", "Point reduction tolerance. Lower = bigger STL.");

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Reset defaults").clicked() {
                *p = Params::default();
                changed = true;
            }
            if ui.button("Save preset").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("cookiecut-preset.json")
                    .add_filter("JSON", &["json"])
                    .save_file()
                {
                    let res = serde_json::to_string_pretty(p)
                        .map_err(|e| e.to_string())
                        .and_then(|s| std::fs::write(&path, s).map_err(|e| e.to_string()));
                    self.status = match res {
                        Ok(()) => format!("Saved {}", path.display()),
                        Err(e) => format!("Save failed: {e}"),
                    };
                }
            }
            if ui.button("Load preset").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
                    match std::fs::read_to_string(&path)
                        .map_err(|e| e.to_string())
                        .and_then(|s| serde_json::from_str::<Params>(&s).map_err(|e| e.to_string()))
                    {
                        Ok(np) => {
                            *p = np;
                            changed = true;
                        }
                        Err(e) => self.status = format!("Load failed: {e}"),
                    }
                }
            }
        });

        if changed {
            self.dirty = true;
        }
    }

    fn preview(&self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::hover());
        let rect = resp.rect;
        painter.rect_filled(rect, 0.0, Color32::from_gray(30));
        let Some(b) = &self.result else { return };

        let [wmm, hmm] = b.size_mm;
        let scale = ((rect.width() - 20.0) / wmm).min((rect.height() - 20.0) / hmm);
        let origin = rect.center() - Vec2::new(wmm, hmm) * scale * 0.5;
        // Image y grows downward. Mirrored geometry keeps that, unmirrored flips it.
        let to_screen = |p: [f64; 2]| -> Pos2 {
            let y = if self.params.mirror { p[1] as f32 } else { hmm - p[1] as f32 };
            origin + Vec2::new(p[0] as f32, y) * scale
        };

        if self.show_seg {
            if let Some(tex) = &self.seg_tex {
                let img_rect = Rect::from_min_size(origin, Vec2::new(wmm, hmm) * scale);
                painter.image(
                    tex.id(),
                    img_rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::from_white_alpha(90),
                );
            }
        }

        let draw = |polys: &[Polygon], color: Color32, width: f32| {
            for poly in polys {
                for ring in std::iter::once(&poly.outer).chain(poly.holes.iter()) {
                    let pts: Vec<Pos2> = ring.iter().map(|p| to_screen(*p)).collect();
                    painter.add(egui::Shape::closed_line(pts, Stroke::new(width, color)));
                }
            }
        };
        if self.show_flange {
            draw(&b.base, Color32::from_rgb(120, 170, 255), 1.0);
        }
        if self.show_blade {
            draw(&b.blade, Color32::from_rgb(255, 90, 90), 1.5);
        }
        if self.show_plate {
            draw(&b.plate, Color32::from_rgb(110, 230, 130), 1.0);
        }
        if self.show_detail {
            draw(&b.rim, Color32::from_rgb(255, 220, 90), 1.0);
            draw(&b.detail, Color32::from_rgb(255, 220, 90), 1.0);
        }

        painter.text(
            rect.left_top() + Vec2::new(8.0, 8.0),
            egui::Align2::LEFT_TOP,
            format!("{:.1} x {:.1} mm (with margin)", wmm, hmm),
            egui::FontId::monospace(12.0),
            Color32::LIGHT_GRAY,
        );
    }
}

fn seg_image(b: &Build) -> egui::ColorImage {
    let (w, h) = (b.seg.w as usize, b.seg.h as usize);
    let mut px = Vec::with_capacity(w * h * 3);
    for (s, d) in b.seg.silhouette.pixels().zip(b.seg.detail.pixels()) {
        let c: [u8; 3] = if d[0] > 0 {
            [60, 40, 30]
        } else if s[0] > 0 {
            [200, 170, 120]
        } else {
            [60, 60, 60]
        };
        px.extend_from_slice(&c);
    }
    egui::ColorImage::from_rgb([w, h], &px)
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "params", &self.params);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect()
        });
        if let Some(p) = dropped.into_iter().next() {
            self.open(p);
        }

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open image…").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp", "gif"])
                        .pick_file()
                    {
                        self.open(p);
                    }
                }
                if ui.button("Export STL…").clicked() {
                    self.export_dialog();
                }
                ui.separator();
                ui.label("Show:");
                ui.checkbox(&mut self.show_seg, "image");
                ui.checkbox(&mut self.show_flange, "flange");
                ui.checkbox(&mut self.show_blade, "blade");
                ui.checkbox(&mut self.show_plate, "plate");
                ui.checkbox(&mut self.show_detail, "detail");
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.label(&self.status);
        });

        egui::SidePanel::left("params")
            .resizable(true)
            .default_width(340.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.params_ui(ui));
            });

        if self.dirty {
            self.rebuild(ctx);
        }

        egui::CentralPanel::default().show(ctx, |ui| self.preview(ui));
    }
}
