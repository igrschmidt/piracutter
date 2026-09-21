#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use cookiecut::{params, pipeline};

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

/// Cookie cutter + stamp STL generator. Run without arguments for the GUI.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Input image (PNG/JPG/WebP/BMP/GIF).
    input: Option<PathBuf>,
    /// Output base path; writes <base>_cutter.stl and <base>_stamp.stl. Requires input.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Preset JSON saved from the GUI.
    #[arg(short, long)]
    config: Option<PathBuf>,
    /// Override cookie size in mm.
    #[arg(short, long)]
    size: Option<f32>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let (Some(input), Some(output)) = (&cli.input, &cli.output) {
        let mut params = match &cli.config {
            Some(c) => serde_json::from_str(&std::fs::read_to_string(c)?)?,
            None => params::Params::default(),
        };
        if let Some(s) = cli.size {
            params.size_mm = s;
        }
        let img = pipeline::load_image(input)?;
        let b = pipeline::build(&img, &params)?;
        for p in pipeline::export(&b, &params, output)? {
            println!("{}", p.display());
        }
        return Ok(());
    }

    let initial = cli.input.clone();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 820.0])
            .with_title("cookiecut"),
        ..Default::default()
    };
    eframe::run_native(
        "cookiecut",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, initial)))),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}
