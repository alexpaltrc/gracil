//! Compresses videos and images from the command line, saving each as `Gracil-<name>` beside the
//! original.
//!
//! ```text
//! cargo run --release -p gracil-core --example compress -- <file>...
//! ```
//!
//! A development aid for the engine; the app itself never shows a console.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

use gracil_core::compress::{Engine, Update, compress};
use gracil_core::encode::JobControl;
use gracil_core::images;
use gracil_core::media::{Kind, kind_of};
use gracil_core::output::save_as_new;
use gracil_core::plan::output_extension;

fn main() {
    // Shows the engine's decisions, such as the quality level each video gets.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    let sources: Vec<_> = std::env::args_os().skip(1).collect();
    if sources.is_empty() {
        eprintln!("usage: compress <file>...");
        std::process::exit(2);
    }
    let Some(engine) = Engine::start() else {
        eprintln!("no usable FFmpeg found");
        std::process::exit(1);
    };
    eprintln!("{:?}", engine.capabilities);

    let mut failed = false;
    for source in sources {
        if let Err(error) = run(&engine, Path::new(&source)) {
            eprintln!("\n{}: {error}", Path::new(&source).display());
            failed = true;
        }
    }
    std::process::exit(i32::from(failed));
}

fn run(engine: &Engine, source: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let before = fs::metadata(source)?.len();
    let started = Instant::now();
    let mut show = |update: Update| {
        if let Some(fraction) = update.fraction {
            eprint!("\r{} {:>5.1}%", source.display(), fraction * 100.0);
            let _ = std::io::stderr().flush();
        }
    };
    let control = JobControl::default();
    let (temp, size, extension, how) = match kind_of(source) {
        Kind::Video => {
            let result = compress(engine, source, &control, &mut show)?;
            let how = format!("{:?}", result.attempt);
            (result.temp, result.size, output_extension(source), how)
        }
        Kind::Image => {
            let result = images::compress(engine, source, &control, &mut show)?;
            (
                result.temp,
                result.size,
                result.extension,
                "image".to_owned(),
            )
        }
    };

    let destination = save_as_new(&temp, source, &extension)?;

    eprintln!(
        "\r{} -> {} ({:.1}% of the original, {:.1} s, {how})",
        source.display(),
        destination.display(),
        size as f64 / before as f64 * 100.0,
        started.elapsed().as_secs_f64(),
    );
    Ok(())
}
