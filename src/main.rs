#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod render;

use piracutter::{params, pipeline};

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

/// Gera ficheiros STL de cortador de bolachas e carimbo a partir de uma imagem.
/// Sem argumentos abre a aplicação com interface.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Imagem de entrada (PNG/JPG/WebP/BMP/GIF).
    input: Option<PathBuf>,
    /// Caminho base de saída; grava <base>_cortador.stl e <base>_carimbo.stl. Exige a imagem de entrada.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Predefinição em JSON guardada na aplicação.
    #[arg(short, long)]
    config: Option<PathBuf>,
    /// Tamanho da bolacha em mm, substituindo o da predefinição.
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
            .with_title("PiraCutter"),
        ..Default::default()
    };
    eframe::run_native(
        "PiraCutter",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, initial)))),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}
