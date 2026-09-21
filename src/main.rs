#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod render;

use piracutter::pipeline::Format;
use piracutter::{params, pipeline};

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

/// Gera cortador de biscoitos e carimbo a partir de uma imagem, em 3MF ou STL.
/// Sem argumentos, abre o aplicativo com interface.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Imagem de entrada (PNG/JPG/WebP/BMP/GIF).
    input: Option<PathBuf>,
    /// Caminho de saída. Terminando em .3mf, salva as duas peças em um só arquivo;
    /// caso contrário salva <base>_cortador.stl e <base>_carimbo.stl.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Predefinição em JSON salva no aplicativo.
    #[arg(short, long)]
    config: Option<PathBuf>,
    /// Tamanho do biscoito em mm, no lugar do valor da predefinição.
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
        for p in pipeline::export(&b, &params, output, Format::from_path(output))? {
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
