pub mod endpoints;
pub mod ffmpeg;
pub mod logging;
pub mod runner;
pub mod service;

use clap::{Parser, Subcommand};
use service::*;
use std::sync::Arc;
use std::{thread, time::Duration};
use tokio::sync::Mutex;

#[derive(Subcommand)]
enum CliCommand {
    /// Record for 10 seconds and quit
    Start {
        #[clap(short, long, default_value = "false")]
        audio: bool,
    },
    /// Start server
    Server {
        /// Net listening address of HTTP server in case of "server" command
        #[clap(short, long, default_value = "0.0.0.0:8000", env = "LISTEN")]
        listen: String,
    },
}

#[derive(Parser)]
#[clap(version = "0.1.0")]
struct Opts {
    #[clap(subcommand)]
    cmd: CliCommand,
}

#[tokio::main]
async fn main() {
    color_eyre::install().unwrap();
    logging::start("INFO");

    let opt = Opts::parse();
    match opt.cmd {
        CliCommand::Server { listen } => {
            let socket_addr: std::net::SocketAddr = listen.parse().expect("invalid bind to listen");
            endpoints::run(socket_addr).await.unwrap();
        }
        CliCommand::Start { audio } => {
            // start recording
            let mx = Arc::new(Mutex::new(RecordingState::Waiting));

            let mx1 = mx.clone();
            let opt = RecordingOptions { audio };
            tokio::spawn(async {
                let _ = start(mx1, opt).await.unwrap();
            });

            let h2 = tokio::spawn(async {
                thread::sleep(Duration::from_secs(10));
                let _ = stop(mx).await.unwrap();
            });
            println!("STATUS: launched, waiting for 10 seconds to stop");
            h2.await.unwrap();
        }
    }
}
