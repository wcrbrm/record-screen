use crate::ffmpeg::*;
use anyhow::bail;
use color_eyre::owo_colors::OwoColorize;
use futures::{future::ready, StreamExt};
use serde::Serialize;
use std::process::Stdio;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum RecordingState {
    Waiting,
    Started {
        #[serde(skip_serializing_if = "Option::is_none")]
        progress: Option<Progress>,
        process_id: u32,
        file: String,
    },
    Stopping {
        process_id: u32,
        file: String,
    },
    Compressing {
        process_id: u32,
        input: String,
        output: String,
    },
    Done {
        file: String,
    },
}

impl RecordingState {
    pub fn set_progress(&mut self, p: Progress) {
        if let Self::Started {
            progress,
            process_id: _,
            file: _,
        } = self
        {
            *progress = Some(p.clone());
        };
    }
}

#[derive(Default, Debug, Clone, serde::Deserialize)]
pub struct RecordingOptions {
    #[serde(default)]
    pub audio: bool,
}

/// start process of recording
pub async fn start(mx: Arc<Mutex<RecordingState>>, opt: RecordingOptions) -> anyhow::Result<()> {
    let current = mx.clone().lock().await.clone();
    match current {
        RecordingState::Done { .. } => {}
        RecordingState::Waiting => {}
        _ => anyhow::bail!("not ready to start"),
    };
    let pictures = dirs::video_dir().unwrap();
    let out = format!(
        "{}/{}.mp4",
        pictures.to_str().unwrap(),
        chrono::Local::now().format("%Y-%m-%dT%H-%M")
    );
    println!("{} {:?} -> {}", "on air".green(), opt, out.yellow());
    let mut builder = FfmpegBuilder::new().stderr(Stdio::piped());
    builder = builder
        .option(Parameter::KeyValue("f", "x11grab"))
        .option(Parameter::KeyValue("video_size", "1920x1080"))
        .option(Parameter::KeyValue("framerate", "25"))
        .option(Parameter::KeyValue("i", ":1.0"));
    if opt.audio {
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
    let process_id = ffmpeg.process.id();
    if process_id > 0 {
        *mx.clone().lock().await = RecordingState::Started {
            progress: None,
            process_id,
            file: out.clone(),
        };
    }
    ffmpeg
        .progress
        .for_each(|x| {
            if let Ok(p) = x {
                println!("{}", p.print_info());
                //  mx.lock().await.set_progress(p);
                //futures::executor::block_on(async move || {
                //   *(mx.clone().lock().await).set_progress(&p);
                //});
            }
            ready(())
        })
        .await;

    Ok(())
}

/// stop process of recording
pub async fn stop(mx: Arc<Mutex<RecordingState>>) -> anyhow::Result<()> {
    let current = mx.clone().lock().await.clone();
    let (pid, input) = if let RecordingState::Started {
        process_id, file, ..
    } = current
    {
        (process_id, file.to_string())
    } else {
        bail!("not started")
    };
    let output = input.clone().replace(".mp4", ".compressed.mp4");

    println!("{} {}", "stopping".green(), pid);
    *mx.clone().lock().await = RecordingState::Stopping {
        process_id: pid,
        file: input.clone(),
    };

    // sending kill signal for a process
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(pid as i32),
        nix::sys::signal::Signal::SIGINT,
    )
    .expect("cannot send ctrl-c");

    // wait for process to be finished if the process is finished
    nix::sys::wait::waitpid(nix::unistd::Pid::from_raw(pid as i32), None).expect("waitpid failed");

    // start compression and watch its progress
    // ffmpeg -i input.mp4 -vcodec libx264 -crf 20 output.mp4
    let mut builder = FfmpegBuilder::new().stderr(Stdio::piped());
    builder = builder
        .input(File::new(&input))
        .option2(Parameter::KeyValue("vcodec", "libx264"))
        .option2(Parameter::KeyValue("crf", "20"))
        .output(File::new(&output));
    let ffmpeg = builder.run().await.unwrap();
    let process_id = ffmpeg.process.id();
    *mx.clone().lock().await = RecordingState::Compressing {
        process_id,
        input: input.clone(),
        output: output.clone(),
    };

    println!("{} {}", "compressing".green(), process_id);
    ffmpeg
        .progress
        .for_each(|x| {
            if let Ok(p) = x {
                println!("{}", p.print_info());
            }
            ready(())
        })
        .await;

    *mx.clone().lock().await = RecordingState::Done {
        file: output.clone(),
    };
    // remove local "input" file, ignore error
    let _ = std::fs::remove_file(input);

    println!("{} {}", "done".green(), output.yellow());
    Ok(())
}
