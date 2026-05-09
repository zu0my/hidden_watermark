use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use hidden_watermark::{
    BackendChoice, DecodeOptions, DecodeStatus, EncodeOptions, RobustWatermarkOptions,
    WatermarkPreset, decode_image, encode_image, estimate_capacity,
};

#[derive(Debug, Parser)]
#[command(version, about = "Robust image watermark encoder and blind decoder")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Encode {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        id: String,
        #[command(flatten)]
        watermark: WatermarkArgs,
        #[arg(long)]
        jpeg_quality: Option<u8>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        output_format: OutputFormat,
    },
    Decode {
        #[arg(long)]
        input: PathBuf,
        #[command(flatten)]
        watermark: WatermarkArgs,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        diagnostic: OutputFormat,
        #[arg(long)]
        no_preprocess: bool,
    },
    Capacity {
        #[arg(long)]
        input: PathBuf,
        #[command(flatten)]
        watermark: WatermarkArgs,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        output_format: OutputFormat,
    },
    Diagnose {
        #[arg(long)]
        input: PathBuf,
        #[command(flatten)]
        watermark: WatermarkArgs,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        output_format: OutputFormat,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliBackend {
    Auto,
    FrequencyV2,
    JpegDct,
}

impl From<CliBackend> for BackendChoice {
    fn from(value: CliBackend) -> Self {
        match value {
            CliBackend::Auto => Self::Auto,
            CliBackend::FrequencyV2 => Self::FrequencyV2,
            CliBackend::JpegDct => Self::JpegDct,
        }
    }
}

#[derive(Clone, Debug, Parser)]
struct WatermarkArgs {
    #[arg(long)]
    key: Option<String>,
    #[arg(long, default_value_t = 0.25)]
    strength: f32,
    #[arg(long, default_value_t = 512)]
    tile_size: u32,
    #[arg(long, default_value_t = 0.0)]
    overlap: f32,
    #[arg(long, value_enum, default_value_t = CliPreset::Invisible)]
    preset: CliPreset,
    #[arg(long, value_enum, default_value_t = CliBackend::Auto)]
    backend: CliBackend,
    #[arg(long, default_value_t = 3)]
    cross_band: usize,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliPreset {
    Invisible,
    Balanced,
    Robust,
}

impl From<CliPreset> for WatermarkPreset {
    fn from(value: CliPreset) -> Self {
        match value {
            CliPreset::Invisible => Self::Invisible,
            CliPreset::Balanced => Self::Balanced,
            CliPreset::Robust => Self::Robust,
        }
    }
}

impl From<WatermarkArgs> for RobustWatermarkOptions {
    fn from(value: WatermarkArgs) -> Self {
        Self {
            key: value.key,
            strength: value.strength,
            tile_size: value.tile_size,
            overlap: value.overlap,
            preset: value.preset.into(),
            cross_band_count: value.cross_band,
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    match cli.command {
        Command::Encode {
            input,
            output,
            id,
            watermark,
            jpeg_quality,
            output_format,
        } => {
            let backend_choice = watermark.backend.into();
            let report = encode_image(
                &input,
                &output,
                &id,
                EncodeOptions {
                    watermark: watermark.into(),
                    jpeg_quality,
                    backend: backend_choice,
                },
            )?;
            match output_format {
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
                OutputFormat::Text => {
                    println!(
                        "encoded id_bytes={} tiles={} image={}x{} tile_size={} strength={}",
                        report.id_bytes,
                        report.tile_count,
                        report.width,
                        report.height,
                        report.tile_size,
                        report.strength
                    );
                    println!(
                        "backend={} algorithm={} psnr={:.2} changed_pixels_ratio={:.4}",
                        report.backend, report.algorithm, report.psnr, report.changed_pixels_ratio
                    );
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Decode {
            input,
            watermark,
            diagnostic,
            no_preprocess,
        } => {
            let json = matches!(diagnostic, OutputFormat::Json);
            let backend_choice = watermark.backend.into();

            let effective_input = if no_preprocess {
                input.clone()
            } else {
                match hidden_watermark::preprocess_with_opencv(&input) {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("warning: preprocessing skipped ({e}), decoding raw image");
                        input.clone()
                    }
                }
            };

            let report = decode_image(
                &effective_input,
                DecodeOptions {
                    watermark: watermark.into(),
                    enable_diagnostics: json,
                    backend: backend_choice,
                },
            )?;
            match diagnostic {
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
                OutputFormat::Text => {
                    if let Some(id) = &report.id {
                        println!(
                            "{id}\nconfidence={:.3} tile_hits={} rotation={} scale={:.2} corrected_bytes={}",
                            report.confidence,
                            report.tile_hits,
                            report.best_rotation_degrees,
                            report.best_scale,
                            report.corrected_bytes
                        );
                    } else {
                        println!("no reliable watermark decoded; status={:?}", report.status);
                    }
                }
            }
            Ok(if report.status == DecodeStatus::Decoded {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            })
        }
        Command::Capacity {
            input,
            watermark,
            output_format,
        } => {
            let report = estimate_capacity(input, watermark.into())?;
            match output_format {
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
                OutputFormat::Text => {
                    println!(
                        "max_id_bytes={} recommended_id_bytes={} tiles={} image={}x{} tile_size={}",
                        report.max_id_bytes,
                        report.recommended_id_bytes,
                        report.tile_count,
                        report.width,
                        report.height,
                        report.tile_size
                    );
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Diagnose {
            input,
            watermark,
            output_format,
        } => {
            let backend_choice = watermark.backend.into();
            let report = decode_image(
                &input,
                DecodeOptions {
                    watermark: watermark.into(),
                    enable_diagnostics: true,
                    backend: backend_choice,
                },
            )?;
            match output_format {
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
                OutputFormat::Text => {
                    println!(
                        "status={:?} id={} confidence={:.3} attempts={} tile_hits={} rotation={} scale={:.2}",
                        report.status,
                        report.id.as_deref().unwrap_or("<none>"),
                        report.confidence,
                        report.attempts,
                        report.tile_hits,
                        report.best_rotation_degrees,
                        report.best_scale
                    );
                    for tile in report.diagnostics.iter().take(20) {
                        println!(
                            "tile x={} y={} rot={} scale={:.2} confidence={:.3} status={:?}",
                            tile.x,
                            tile.y,
                            tile.rotation_degrees,
                            tile.scale,
                            tile.confidence,
                            tile.status
                        );
                    }
                }
            }
            Ok(if report.status == DecodeStatus::Decoded {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            })
        }
    }
}
