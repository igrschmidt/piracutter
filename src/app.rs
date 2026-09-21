use crate::render::{self, Camera, Part};
use piracutter::geometry::Polygon;
use piracutter::params::{BgMode, Params, SizeMode};
use piracutter::pipeline::{build, cutter_tris, export, load_image, stamp_tris, Build};
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
enum View {
    Outline,
    Solid,
}

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

    view: View,
    camera: Camera,
    parts: Vec<Part>,
    solid_tex: Option<egui::TextureHandle>,
    /// Bumped on every rebuild so a cached frame knows it is stale.
    mesh_gen: u64,
    raster_key: Option<(u64, [usize; 2], [f32; 6])>,
    side_by_side: bool,
    show_cutter_3d: bool,
    show_stamp_3d: bool,
    show_plate_grid: bool,
    /// Framing needs the viewport shape, so it waits for the next paint.
    fit_pending: bool,
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
            status: "Abra uma imagem com o botão Abrir imagem, ou largue o ficheiro nesta janela.".into(),
            dirty: false,
            show_seg: true,
            show_blade: true,
            show_flange: true,
            show_plate: true,
            show_detail: true,

            view: View::Solid,
            camera: Camera::default(),
            parts: Vec::new(),
            solid_tex: None,
            mesh_gen: 0,
            raster_key: None,
            side_by_side: true,
            show_cutter_3d: true,
            show_stamp_3d: true,
            show_plate_grid: true,
            fit_pending: false,
        };
        if let Some(p) = initial {
            app.open(p);
        }
        app
    }

    fn open(&mut self, path: PathBuf) {
        match load_image(&path) {
            Ok(img) => {
                self.status = format!("Imagem carregada: {} ({}x{})", path.display(), img.width(), img.height());
                self.image = Some(img);
                self.image_path = Some(path);
                self.dirty = true;
            }
            Err(e) => self.status = format!("Erro: {e:#}"),
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
                    "Gerado em {} ms. Lâmina {}, base {}, placa {}, detalhe {} polígonos.",
                    t.elapsed().as_millis(),
                    b.blade.len(),
                    b.base.len(),
                    b.plate.len(),
                    b.detail.len()
                ) + &match (b.relaxed, b.dropped) {
                    (0, 0) => String::new(),
                    (r, 0) => format!(" Suavização reduzida ({r}) para manter os contornos afastados."),
                    (0, d) => format!(" {d} forma(s) demasiado emaranhada(s) para gerar sólido, deixada(s) de fora."),
                    (r, d) => format!(
                        " Suavização reduzida ({r}); {d} forma(s) demasiado emaranhada(s) para gerar sólido, deixada(s) de fora."
                    ),
                };
                let first = self.parts.is_empty();
                self.result = Some(b);
                self.rebuild_mesh();
                self.fit_pending |= first;
            }
            Err(e) => self.status = format!("Erro: {e:#}"),
        }
    }

    fn export_dialog(&mut self) {
        let Some(b) = &self.result else {
            self.status = "Nada para exportar.".into();
            return;
        };
        let suggested = self
            .image_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("bolacha")
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
                    self.status = format!("Ficheiros gravados: {}", names.join(", "));
                }
                Err(e) => self.status = format!("Falha ao exportar: {e:#}"),
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

        ui.heading("Tamanho");
        s!(ui, p.size_mm, 15.0..=250.0, "Tamanho da bolacha", " mm", "Medido no próprio desenho. O espaço vazio à volta da imagem é ignorado, e a aba acrescenta a este valor.");
        egui::ComboBox::from_label("Medido em")
            .selected_text(match p.size_mode {
                SizeMode::Width => "largura",
                SizeMode::Height => "altura",
                SizeMode::Longest => "maior lado",
            })
            .show_ui(ui, |ui| {
                for (m, label) in [
                    (SizeMode::Longest, "maior lado"),
                    (SizeMode::Width, "largura"),
                    (SizeMode::Height, "altura"),
                ] {
                    changed |= ui.selectable_value(&mut p.size_mode, m, label).changed();
                }
            });
        s!(ui, p.px_per_mm, 3.0..=20.0, "Resolução", " px/mm", "Mais alto dá curvas mais suaves e demora mais. 8 chega bem para impressão.");
        changed |= ui
            .checkbox(&mut p.mirror, "Espelhar geometria")
            .on_hover_text("Deixe ligado. As peças são viradas ao carimbar a massa, por isso o modelo tem de ser a imagem espelhada.")
            .changed();

        ui.separator();
        ui.heading("Imagem");
        egui::ComboBox::from_label("Fundo")
            .selected_text(bg_label(p.bg_mode))
            .show_ui(ui, |ui| {
                for m in [BgMode::Auto, BgMode::Alpha, BgMode::BorderColor] {
                    changed |= ui.selectable_value(&mut p.bg_mode, m, bg_label(m)).changed();
                }
            });
        s!(ui, p.bg_tolerance, 0.01..=0.6, "Tolerância do fundo", "", "Quanto uma cor pode diferir da cor das margens da imagem e ainda contar como fundo.");
        changed |= ui
            .checkbox(&mut p.keep_holes, "Manter buracos fechados")
            .on_hover_text("Desligado: o fundo preso dentro da forma passa a fazer parte da bolacha. Ligado: passa a ser um buraco com lâmina própria.")
            .changed();
        s!(ui, p.detail_threshold, 0..=255, "Escuridão do detalhe", "", "Os pixels mais escuros do que este valor passam a linhas em relevo no carimbo.");
        s!(ui, p.min_blob_mm2, 0.0..=20.0, "Área mínima da forma", " mm²", "Descarta salpicos da silhueta menores do que isto.");
        s!(ui, p.min_detail_mm2, 0.0..=5.0, "Área mínima do detalhe", " mm²", "Descarta salpicos de detalhe menores do que isto.");

        ui.separator();
        changed |= ui.checkbox(&mut p.cutter_enabled, "Cortador").changed();
        ui.add_enabled_ui(p.cutter_enabled, |ui| {
            s!(ui, p.blade_thickness, 0.4..=3.0, "Espessura da lâmina", " mm", "Duas larguras de bico (0,8) imprimem uma lâmina limpa de duas paredes.");
            s!(ui, p.blade_height, 5.0..=40.0, "Altura da lâmina", " mm", "");
            s!(ui, p.blade_offset, 0.0..=3.0, "Afastamento da lâmina", " mm", "Folga entre o contorno da silhueta e a face interior da lâmina.");
            s!(ui, p.flange_width, 0.0..=15.0, "Largura da aba", " mm", "Rebordo para fora na base, onde faz pressão com a mão.");
            s!(ui, p.flange_height, 0.4..=6.0, "Altura da aba", " mm", "");
            s!(ui, p.inner_lip_width, 0.0..=5.0, "Largura do rebordo interior", " mm", "Rebordo opcional por dentro da lâmina, à altura da aba. Dá rigidez em formas estreitas.");
        });

        ui.separator();
        changed |= ui.checkbox(&mut p.stamp_enabled, "Carimbo").changed();
        ui.add_enabled_ui(p.stamp_enabled, |ui| {
            s!(ui, p.plate_thickness, 1.0..=10.0, "Espessura da placa", " mm", "");
            s!(ui, p.plate_clearance, 0.0..=4.0, "Folga da placa", " mm", "Quanto a placa é mais pequena do que o cortador, para entrar dentro da lâmina.");
            s!(ui, p.detail_height, 0.4..=5.0, "Altura do detalhe", " mm", "Quanto as linhas sobressaem da placa.");
            s!(ui, p.detail_expand, -0.5..=1.5, "Engrossar detalhe", " mm", "Engrossa as linhas do detalhe, ou afina-as com valores negativos. Linhas com menos de 0,8 mm não saem na impressão.");
            s!(ui, p.rim_width, 0.0..=4.0, "Rebordo do contorno", " mm", "Faixa em relevo a acompanhar a berma da placa, para a bolacha ficar com o contorno marcado. Zero desliga.");
            s!(ui, p.detail_inset, 0.0..=3.0, "Recuo do detalhe", " mm", "Mantém o resto do detalhe a esta distância por dentro do rebordo.");
        });

        ui.separator();
        ui.heading("Curvas");
        s!(ui, p.smooth_iters, 0..=4, "Suavização", "", "Passagens de Chaikin sobre os contornos traçados.");
        s!(ui, p.simplify_mm, 0.0..=0.3, "Simplificação", " mm", "Tolerância na redução de pontos. Menor dá STL maior.");

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Repor predefinições").clicked() {
                *p = Params::default();
                changed = true;
            }
            if ui.button("Guardar predefinição").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("piracutter-predefinicao.json")
                    .add_filter("JSON", &["json"])
                    .save_file()
                {
                    let res = serde_json::to_string_pretty(p)
                        .map_err(|e| e.to_string())
                        .and_then(|s| std::fs::write(&path, s).map_err(|e| e.to_string()));
                    self.status = match res {
                        Ok(()) => format!("Predefinição guardada: {}", path.display()),
                        Err(e) => format!("Falha ao guardar: {e}"),
                    };
                }
            }
            if ui.button("Carregar predefinição").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
                    match std::fs::read_to_string(&path)
                        .map_err(|e| e.to_string())
                        .and_then(|s| serde_json::from_str::<Params>(&s).map_err(|e| e.to_string()))
                    {
                        Ok(np) => {
                            *p = np;
                            changed = true;
                        }
                        Err(e) => self.status = format!("Falha ao carregar: {e}"),
                    }
                }
            }
        });

        if changed {
            self.dirty = true;
        }
    }

    fn rebuild_mesh(&mut self) {
        self.parts.clear();
        self.mesh_gen = self.mesh_gen.wrapping_add(1);
        self.raster_key = None;
        let Some(b) = &self.result else { return };

        let mut parts = Vec::new();
        if self.params.cutter_enabled {
            if let Ok(tris) = cutter_tris(b, &self.params) {
                parts.push(Part {
                    name: "cutter",
                    tris,
                    color: [176.0, 190.0, 214.0],
                    offset: [0.0; 3],
                });
            }
        }
        if self.params.stamp_enabled {
            if let Ok(tris) = stamp_tris(b, &self.params) {
                parts.push(Part {
                    name: "stamp",
                    tris,
                    color: [226.0, 196.0, 148.0],
                    offset: [0.0; 3],
                });
            }
        }

        if self.side_by_side && parts.len() == 2 {
            let edge = |p: &Part, pick: fn(f32, f32) -> f32, init: f32| {
                p.tris.iter().flatten().map(|v| v[0]).fold(init, pick)
            };
            let cutter_right = edge(&parts[0], f32::max, f32::MIN);
            let stamp_left = edge(&parts[1], f32::min, f32::MAX);
            parts[1].offset[0] = cutter_right - stamp_left + 8.0;
        }

        // Centre the scene over the plate so orbiting feels natural.
        if let Some((lo, hi)) = render::bbox(parts.iter()) {
            let centre = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5];
            for p in &mut parts {
                p.offset[0] -= centre[0];
                p.offset[1] -= centre[1];
            }
        }
        self.parts = parts;
    }

    fn visible_parts(&self) -> impl Iterator<Item = &Part> {
        let (cutter, stamp) = (self.show_cutter_3d, self.show_stamp_3d);
        self.parts.iter().filter(move |p| match p.name {
            "cutter" => cutter,
            _ => stamp,
        })
    }

    fn fit_view(&mut self, aspect: f32) {
        match render::bbox(self.visible_parts()) {
            Some(bb) => self.camera.frame(bb, aspect),
            None => self.camera = Camera::default(),
        }
        self.raster_key = None;
    }

    fn solid_view(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = resp.rect;
        painter.rect_filled(rect, 0.0, Color32::from_gray(26));

        if self.parts.is_empty() {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Abra uma imagem para ver a pré-visualização em sólido.",
                egui::FontId::proportional(14.0),
                Color32::from_gray(140),
            );
            return;
        }

        if std::mem::take(&mut self.fit_pending) {
            self.fit_view(rect.width() / rect.height().max(1.0));
        }

        if resp.dragged_by(egui::PointerButton::Primary) {
            let d = resp.drag_delta();
            self.camera.yaw -= d.x * 0.01;
            self.camera.pitch = (self.camera.pitch + d.y * 0.01).clamp(-1.45, 1.45);
            self.raster_key = None;
        }
        if resp.dragged_by(egui::PointerButton::Secondary)
            || resp.dragged_by(egui::PointerButton::Middle)
        {
            let d = resp.drag_delta();
            let basis = self.camera.basis(rect.width(), rect.height());
            let k = self.camera.dist * 0.0015;
            self.camera.target = basis.pan(self.camera.target, -d.x * k, d.y * k);
            self.raster_key = None;
        }
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.01 {
                self.camera.dist = (self.camera.dist * (1.0 - scroll * 0.002)).clamp(5.0, 5000.0);
                self.raster_key = None;
            }
        }
        if resp.double_clicked() {
            self.fit_view(rect.width() / rect.height().max(1.0));
        }

        if self.show_plate_grid {
            for (seg, color) in
                render::plate_lines(&self.camera, rect.width(), rect.height(), 120.0, 10.0)
            {
                painter.line_segment(
                    [rect.min + seg[0].to_vec2(), rect.min + seg[1].to_vec2()],
                    Stroke::new(1.0_f32, color),
                );
            }
        }

        // Half resolution while the camera moves keeps dragging responsive.
        let ppp = ui.ctx().pixels_per_point();
        let quality = if resp.dragged() { 0.5 } else { 1.0 };
        let w = ((rect.width() * ppp * quality) as usize).clamp(16, 1800);
        let h = ((rect.height() * ppp * quality) as usize).clamp(16, 1400);
        let key = (
            self.mesh_gen,
            [w, h],
            [
                self.camera.yaw,
                self.camera.pitch,
                self.camera.dist,
                self.camera.target[0],
                self.camera.target[1],
                self.camera.target[2],
            ],
        );
        if self.raster_key != Some(key) || self.solid_tex.is_none() {
            let img = render::rasterize(self.visible_parts(), &self.camera, w, h);
            self.solid_tex =
                Some(ui.ctx().load_texture("solid", img, egui::TextureOptions::LINEAR));
            self.raster_key = Some(key);
        }
        if let Some(tex) = &self.solid_tex {
            painter.image(
                tex.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }

        painter.text(
            rect.left_bottom() + Vec2::new(8.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            "arrastar para rodar · arrastar com o botão direito para deslocar · roda do rato para ampliar · duplo clique para enquadrar",
            egui::FontId::proportional(11.0),
            Color32::from_gray(130),
        );
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
            format!("{:.1} x {:.1} mm (com margem)", wmm, hmm),
            egui::FontId::monospace(12.0),
            Color32::LIGHT_GRAY,
        );
    }
}

fn bg_label(m: BgMode) -> &'static str {
    match m {
        BgMode::Auto => "automático",
        BgMode::Alpha => "transparência",
        BgMode::BorderColor => "cor das margens",
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
                if ui.button("Abrir imagem…").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("Imagens", &["png", "jpg", "jpeg", "webp", "bmp", "gif"])
                        .pick_file()
                    {
                        self.open(p);
                    }
                }
                if ui.button("Exportar STL…").clicked() {
                    self.export_dialog();
                }
                ui.separator();
                ui.selectable_value(&mut self.view, View::Solid, "3D");
                ui.selectable_value(&mut self.view, View::Outline, "Contornos");
                ui.separator();
                match self.view {
                    View::Solid => {
                        if ui.button("Enquadrar").clicked() {
                            self.fit_pending = true;
                        }
                        ui.checkbox(&mut self.show_cutter_3d, "cortador");
                        ui.checkbox(&mut self.show_stamp_3d, "carimbo");
                        ui.checkbox(&mut self.show_plate_grid, "grelha");
                        if ui
                            .checkbox(&mut self.side_by_side, "lado a lado")
                            .on_hover_text("Desligado encaixa o carimbo dentro do cortador, como as peças assentam uma na outra.")
                            .changed()
                        {
                            self.rebuild_mesh();
                        }
                        self.raster_key = None;
                    }
                    View::Outline => {
                        ui.label("Mostrar:");
                        ui.checkbox(&mut self.show_seg, "imagem");
                        ui.checkbox(&mut self.show_flange, "base");
                        ui.checkbox(&mut self.show_blade, "lâmina");
                        ui.checkbox(&mut self.show_plate, "placa");
                        ui.checkbox(&mut self.show_detail, "detalhe");
                    }
                }
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

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| match self.view {
                View::Solid => self.solid_view(ui),
                View::Outline => self.preview(ui),
            });
    }
}
