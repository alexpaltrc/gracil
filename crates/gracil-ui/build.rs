//! Links Gracil's icons into the executable.

fn main() {
    println!("cargo:rerun-if-changed=gracil.rc");
    println!("cargo:rerun-if-changed=icons/gracil-app-dark.ico");
    println!("cargo:rerun-if-changed=icons/gracil-app-light.ico");
    if let Err(problem) =
        embed_resource::compile("gracil.rc", embed_resource::NONE).manifest_optional()
    {
        panic!("could not link Gracil's icon: {problem}");
    }
}
