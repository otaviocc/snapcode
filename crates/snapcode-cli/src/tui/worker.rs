// SPDX-License-Identifier: MIT
//! Background threads that render previews and encode them for the terminal.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use ratatui_image::errors::Errors;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::thread::{ResizeRequest, ResizeResponse};
use snapcode_core::config::RenderConfig;
use snapcode_core::{RenderRequest, Renderer};

pub struct Job {
    pub generation: u64,
    pub source: String,
    pub path: Option<PathBuf>,
    pub config: RenderConfig,
}

pub struct Rendered {
    pub protocol: StatefulProtocol,
    pub width: u32,
    pub height: u32,
}

pub struct Done {
    pub generation: u64,
    pub result: Result<Rendered, String>,
}

pub struct Worker {
    jobs: Sender<Job>,
    results: Receiver<Done>,
    encoder: Sender<ResizeRequest>,
    encoded: Receiver<Result<ResizeResponse, Errors>>,
}

impl Worker {
    pub fn spawn(renderer: Arc<Renderer>, picker: Picker) -> Self {
        let (jobs, job_rx) = mpsc::channel::<Job>();
        let (done_tx, results) = mpsc::channel::<Done>();

        thread::spawn(move || {
            while let Ok(mut job) = job_rx.recv() {
                while let Ok(newer) = job_rx.try_recv() {
                    job = newer;
                }
                let done = Done {
                    generation: job.generation,
                    result: render(&renderer, &picker, &job),
                };
                if done_tx.send(done).is_err() {
                    break;
                }
            }
        });

        let (encoder, request_rx) = mpsc::channel::<ResizeRequest>();
        let (encoded_tx, encoded) = mpsc::channel();

        thread::spawn(move || {
            while let Ok(request) = request_rx.recv() {
                if encoded_tx.send(request.resize_encode()).is_err() {
                    break;
                }
            }
        });

        Self {
            jobs,
            results,
            encoder,
            encoded,
        }
    }

    pub fn encoder(&self) -> Sender<ResizeRequest> {
        self.encoder.clone()
    }

    pub fn submit(&self, job: Job) {
        self.jobs.send(job).ok();
    }

    pub fn latest(&self) -> Option<Done> {
        let mut latest = None;
        while let Ok(done) = self.results.try_recv() {
            latest = Some(done);
        }
        latest
    }

    pub fn encoded(&self) -> impl Iterator<Item = Result<ResizeResponse, Errors>> + '_ {
        self.encoded.try_iter()
    }
}

fn render(renderer: &Renderer, picker: &Picker, job: &Job) -> Result<Rendered, String> {
    let mut request = RenderRequest::new(&job.source, &job.config);
    if let Some(path) = &job.path {
        request = request.with_path(path.clone());
    }
    let raster = renderer
        .render_raster(&request)
        .map_err(|error| format!("{error:#}"))?;
    let (width, height) = (raster.width, raster.height);
    let buffer = image::RgbaImage::from_raw(width, height, raster.pixels)
        .ok_or_else(|| "the preview image was the wrong size".to_string())?;
    Ok(Rendered {
        protocol: picker.new_resize_protocol(image::DynamicImage::ImageRgba8(buffer)),
        width,
        height,
    })
}
