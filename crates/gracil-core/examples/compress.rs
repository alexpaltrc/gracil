//! Compresses videos from the command line, saving each as `Gracil_<name>` beside the original.
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
use gracil_core::naming::first_free_save_as_new_path;
use gracil_core::plan::Container;
use gracil_core::toolchain;

fn main() {
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

    let named_like = source.with_extension(Container::output_extension(source));
    let destination =
        first_free_save_as_new_path(&named_like, Path::exists).ok_or("no free file name")?;
    // A hard link fails instead of replacing anything that appeared since the name was chosen.
    fs::hard_link(&result.temp, &destination)?;
    fs::remove_file(&result.temp)?;

    eprintln!(
        "\r{} -> {} ({:.0}% smaller, {:?})",
        source.display(),
        destination.display(),
        (1.0 - result.size as f64 / before as f64) * 100.0,
        result.attempt
    );
    Ok(())
}
