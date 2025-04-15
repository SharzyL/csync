use clap::{CommandFactory, ValueEnum};
use clap_complete::{
    Shell, generate_to,
};
use std::env;
use std::io::Error;

include!("src/args.rs");

fn main() -> Result<(), Error> {
    let outdir = match env::var_os("COMPLETION_OUT_DIR") {
        None => return Ok(()),
        Some(outdir) => PathBuf::from(outdir),
    };

    if !outdir.exists() {
        std::fs::create_dir_all(&outdir)?;
    }

    let name = CsyncArgs::command().get_name().to_string();

    for shell in Shell::value_variants() {
        let path = generate_to(*shell, &mut CsyncArgs::command(), &name, &outdir)?;
        println!("cargo:warning=generated completion file in {path:?}");
    }

    Ok(())
}
