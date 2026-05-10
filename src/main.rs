use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "hidden_watermark")]
#[command(about = "Robust image watermark encoder and non-blind detector")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Embed watermark into image
    Embed {
        /// Input image path
        #[arg(long)]
        input: PathBuf,

        /// Output image path
        #[arg(long)]
        output: PathBuf,

        /// Secret key
        #[arg(long)]
        key: String,

        /// Embedding strength (default: 0.5)
        #[arg(long, default_value = "0.5")]
        strength: f64,
    },

    /// Detect watermark by comparing suspect to original
    Detect {
        /// Original image path
        #[arg(long)]
        original: PathBuf,

        /// Suspect image path
        #[arg(long)]
        suspect: PathBuf,

        /// Secret key
        #[arg(long)]
        key: String,

        /// False positive rate (default: 0.001)
        #[arg(long, default_value = "0.001")]
        fpr: f64,
    },

    /// Batch detection: compare directories of images
    DetectBatch {
        /// Directory with original images
        #[arg(long)]
        original_dir: PathBuf,

        /// Directory with suspect images
        #[arg(long)]
        suspect_dir: PathBuf,

        /// Secret key
        #[arg(long)]
        key: String,

        /// False positive rate (default: 0.001)
        #[arg(long, default_value = "0.001")]
        fpr: f64,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Embed {
            input,
            output,
            key,
            strength,
        } => {
            let image = match hidden_watermark::load_image(&input) {
                Ok(img) => img,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };

            println!("Image size: {}x{}", image.width(), image.height());

            let (watermarked, psnr) = hidden_watermark::embed_watermark(&image, &key, strength);

            if let Err(e) = hidden_watermark::save_image(&output, &watermarked) {
                eprintln!("Error saving: {}", e);
                std::process::exit(1);
            }

            println!("Watermark embedded. PSNR: {:.2} dB", psnr);
            if psnr < 50.0 {
                println!(
                    "WARNING: PSNR {:.2} dB < 50 dB. Consider reducing strength.",
                    psnr
                );
            } else {
                println!("PSNR > 50 dB: Watermark is invisible.");
            }
        }

        Commands::Detect {
            original,
            suspect,
            key,
            fpr,
        } => {
            let original_img = match hidden_watermark::load_image(&original) {
                Ok(img) => img,
                Err(e) => {
                    eprintln!("Error loading original: {}", e);
                    std::process::exit(1);
                }
            };

            let suspect_img = match hidden_watermark::load_image(&suspect) {
                Ok(img) => img,
                Err(e) => {
                    eprintln!("Error loading suspect: {}", e);
                    std::process::exit(1);
                }
            };

            println!(
                "Original: {} ({}x{})",
                original.display(),
                original_img.width(),
                original_img.height()
            );
            println!(
                "Suspect:  {} ({}x{})",
                suspect.display(),
                suspect_img.width(),
                suspect_img.height()
            );
            println!();

            let result = hidden_watermark::detect_watermark(&original_img, &suspect_img, &key, fpr);

            println!(
                "Alignment: rotation={:.1}°, scale={:.3}x, confidence={:.2}",
                result.alignment.rotation, result.alignment.scale, result.alignment.confidence
            );
            println!();
            println!("Score:     {:.4}", result.score);
            println!("Threshold: {:.4}", result.threshold);
            println!();

            if result.alignment.confidence < 0.2 {
                println!("WARNING: Alignment confidence too low, results are unreliable");
                println!();
            }

            if result.detected {
                println!(
                    "RESULT: WATERMARK DETECTED (confidence: {:.2}x threshold)",
                    result.score / result.threshold
                );
            } else {
                println!("RESULT: No watermark detected");
            }
        }

        Commands::DetectBatch {
            original_dir,
            suspect_dir,
            key,
            fpr,
        } => {
            let originals = find_images(&original_dir);
            let suspects = find_images(&suspect_dir);

            let mut matches = Vec::new();
            for (name, orig_path) in &originals {
                if let Some(suspect_path) = suspects.get(name) {
                    matches.push((name.clone(), orig_path.clone(), suspect_path.clone()));
                }
            }

            if matches.is_empty() {
                println!("No matching images found between directories.");
                std::process::exit(1);
            }

            println!("Found {} matching image pairs", matches.len());
            println!();

            let mut detected_count = 0;

            for (name, orig_path, suspect_path) in &matches {
                print!("Processing: {} ... ", name);

                let original_img = match hidden_watermark::load_image(orig_path) {
                    Ok(img) => img,
                    Err(e) => {
                        println!("ERROR: {}", e);
                        continue;
                    }
                };

                let suspect_img = match hidden_watermark::load_image(suspect_path) {
                    Ok(img) => img,
                    Err(e) => {
                        println!("ERROR: {}", e);
                        continue;
                    }
                };

                let result =
                    hidden_watermark::detect_watermark(&original_img, &suspect_img, &key, fpr);

                let align_warn = if result.alignment.confidence < 0.2 {
                    " [LOW ALIGNMENT CONFIDENCE]"
                } else {
                    ""
                };

                if result.detected {
                    detected_count += 1;
                    println!(
                        "DETECTED (score={:.4}, confidence={:.2}x){}",
                        result.score,
                        result.score / result.threshold,
                        align_warn,
                    );
                } else {
                    println!("NOT_DETECTED (score={:.4}){}", result.score, align_warn,);
                }
            }

            println!();
            println!("============================================================");
            println!("SUMMARY");
            println!("============================================================");
            println!("Total pairs:     {}", matches.len());
            println!("Detected:        {}", detected_count);
            println!("Not detected:    {}", matches.len() - detected_count);
            println!(
                "Detection rate:  {:.1}%",
                detected_count as f64 / matches.len() as f64
            );
        }
    }
}

fn find_images(dir: &PathBuf) -> std::collections::HashMap<String, PathBuf> {
    let mut images = std::collections::HashMap::new();
    let extensions = ["jpg", "jpeg", "png", "bmp", "tiff", "webp"];

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if extensions.contains(&ext_str.as_str())
                    && let Some(name) = path.file_stem()
                {
                    images.insert(name.to_string_lossy().to_string(), path);
                }
            }
        }
    }

    images
}
