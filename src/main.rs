//! Omarchy Quattro backend for the WireView Pro II GPU power monitor.
//!
//! Reads the WireView2 app's StatusNotifierItem over DBus (the app violates
//! the SNI spec so strict tray hosts reject it) and manages the app's process
//! lifecycle for the QML frontend.

use clap::{Parser, Subcommand};

use wireview_pro2_qs::{current, open, sni, watch};

#[derive(Parser)]
#[command(
    name = "wireview-pro2-qs",
    version,
    about = "Backend for the Omarchy WireView Pro II bar widget",
    long_about = "Reads the live power reading of the Thermal Grizzly WireView Pro II \
                  from the WireView2 app's DBus item and manages the app's process \
                  lifecycle for the Omarchy Quattro widget."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print one status report as a single JSON line and exit
    Status,
    /// Stream status reports as JSON lines, one per change (1 Hz poll)
    Watch,
    /// Ensure the app is running and its window is visible
    Open,
    /// Kill every WireView instance and start a fresh one
    Restart,
    /// Kill every WireView instance
    Quit,
}

fn main() {
    match Cli::parse().command {
        Command::Status => {
            let connection = sni::session().ok();
            let status = current::current_status(connection.as_ref());
            println!(
                "{}",
                serde_json::to_string(&status).expect("status serializes")
            );
        }
        Command::Watch => watch::watch(),
        Command::Open => {
            let outcome = open::open();
            match outcome {
                Ok(value) => println!("{value:?}"),
                Err(error) => {
                    eprintln!("open failed: {error}");
                    std::process::exit(1);
                }
            }
        }
        Command::Restart => {
            if let Err(error) = open::restart() {
                eprintln!("restart failed: {error}");
                std::process::exit(1);
            }
        }
        Command::Quit => open::quit(),
    }
}
