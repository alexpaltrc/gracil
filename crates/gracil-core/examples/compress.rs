//! Compresses videos from the command line, saving each as `Gracil-<name>` beside the original.
//!
//! ```text
//! cargo run --release -p gracil-core --example compress -- <video>...
//! ```
//!
//! A development aid for the engine; the app itself never shows a console.

use std::fs;
use std::io::Write;
use std::path::Path;

use gracil_core::compress::{Engine, compress};
use gracil_core::encode::JobControl;
use gracil_core::output::save_as_new;
use gracil_core::toolchain;

fn main() {
    // Shows the engine's decisions, such as the quality level each video gets.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    let sources: Vec<_> = std::env::args_os().skip(1).collect();
    if sources.is_empty() {
        eprintln!("usage: compress <video>...");
        std::process::exit(2);
    }
    let Some(private_dir) = toolchain::default_private_dir() else {
        eprintln!("LOCALAPPDATA is not set");
        std::process::exit(1);
    };
    let engine = match Engine::start(&private_dir) {
        Ok(engine) => engine,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
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
    let result = compress(engine, source, &JobControl::default(), &mut |update| {
        if let Some(fraction) = update.fraction {
            eprint!("\r{} {:>5.1}%", source.display(), fraction * 100.0);
            let _ = std::io::stderr().flush();
        }
    })?;

    let destination = save_as_new(&result.temp, source)?;

    eprintln!(
        "\r{} -> {} ({:.0}% smaller, {:?})",
        source.display(),
        destination.display(),
        (1.0 - result.size as f64 / before as f64) * 100.0,
        result.attempt
    );
    Ok(())
}
