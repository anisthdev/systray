mod cli;
mod daemon;
mod protocol;
mod sni;

use std::env;

fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "error");
    }
    env_logger::init();

    if let Err(e) = run() {
        eprintln!("tray: {}", e);
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "daemon" {
        daemon::run()?;
    } else {
        cli::run()?;
    }

    Ok(())
}
