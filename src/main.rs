pub mod ffmpeg;
pub mod runner;

use clap::{Parser, Subcommand};
use color_eyre::owo_colors::OwoColorize;
use ffmpeg::*;
use futures::{future::ready, StreamExt};
use std::process::Stdio;
use std::{thread, time::Duration};

#[derive(Subcommand)]
enum CliCommand {
    /// Start recording
    Start {
        #[clap(short, long, default_value = "true")]
        screen: bool,
        #[clap(short, long, default_value = "false")]
        audio: bool,
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
    let opt = Opts::parse();
    let pictures = dirs::video_dir().unwrap();
    match opt.cmd {
        CliCommand::Start { screen, audio } => {
            let out = format!(
                "{}/{}.mp4",
                pictures.to_str().unwrap(),
                chrono::Local::now().format("%Y-%m-%dT%H-%M")
            );
            println!(
                "{} {screen:} {audio:} -> {}",
                "on air".green(),
                out.yellow()
            );
            let mut builder = FfmpegBuilder::new().stderr(Stdio::piped());
            if screen {
                builder = builder
                    .option(Parameter::KeyValue("f", "x11grab"))
                    .option(Parameter::KeyValue("video_size", "1920x1080"))
                    .option(Parameter::KeyValue("framerate", "25"))
                    .option(Parameter::KeyValue("i", ":1.0"));
            }
            if audio {
                builder = builder
                    .option(Parameter::KeyValue("f", "pulse"))
                    .option(Parameter::KeyValue("ac", "2"))
                    .option(Parameter::KeyValue("i", "default"));
            }

            builder = builder
                .option(Parameter::KeyValue("preset", "ultrafast"))
                .option(Parameter::KeyValue("qp", "0"))
                .option(Parameter::KeyValue("pix_fmt", "yuv444p"))
                .output(File::new(&out));

            let ffmpeg = builder.run().await.unwrap();
            let child = ffmpeg.process;
            let h = thread::spawn(move || {
                thread::sleep(Duration::from_secs(10));
                // sending ctrl+c signal
                nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(child.id() as i32),
                    nix::sys::signal::Signal::SIGINT,
                )
                .expect("cannot send ctrl-c");
                // thread::sleep(Duration::from_secs(1)); // wait one more second
            });
            ffmpeg
                .progress
                .for_each(|x| {
                    if let Ok(p) = x {
                        println!("{}", p.print_info());
                    }
                    ready(())
                })
                .await;
            h.join().expect("join fail");
        }
    }
}
