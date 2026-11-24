//! Links Gracil's icons into the executable: Gracil Dev's, with their "Dev" band, in a
//! development build or one made with the `dev` feature (ADR 0082).

fn main() {
    println!("cargo:rerun-if-changed=gracil.rc");
    println!("cargo:rerun-if-changed=icons");
    let dev = std::env::var_os("CARGO_FEATURE_DEV").is_some()
        || std::env::var("PROFILE").is_ok_and(|profile| profile == "debug");
    let macros: &[&str] = if dev { &["GRACIL_DEV"] } else { &[] };
    if let Err(problem) = embed_resource::compile("gracil.rc", macros).manifest_optional() {
        panic!("could not link Gracil's icon: {problem}");
    }
}
