use std::error::Error;

use clap::Parser;

use fits_cli::r#struct::Struct;

// Avoid musl's default allocator due to lackluster performance
// https://nickb.dev/blog/default-musl-allocator-considered-harmful-to-performance
#[cfg(all(target_env = "musl", target_arch = "x86_64"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Perform FITS file related operations on the command line.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
enum Args {
    /// Read and print the structure of a FITS file
    #[clap(name = "struct")]
    Struct(Struct),
    /*/// Read and print the headers of all the HDU in a FITS file
    #[clap(name = "head")]
    Head(Head),*/
}

impl Args {
    fn exec(self) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Struct(args) => args.exec(),
            // Self::Head(args) => args.exec(),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let args = Args::parse();
    args.exec()
}
