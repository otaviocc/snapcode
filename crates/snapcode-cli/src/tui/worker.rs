// SPDX-License-Identifier: MIT
//! A background thread that renders previews so the event loop never waits on one.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use snapcode_core::config::RenderConfig;
use snapcode_core::{RenderRequest, Renderer};

pub struct Job {
    pub generation: u64,
    pub source: String,
    pub path: Option<PathBuf>,
    pub config: RenderConfig,
}

pub struct Done {
    pub generation: u64,
    pub result: Result<image::RgbaImage, String>,
}

pub struct Worker {
    jobs: Sender<Job>,
    results: Receiver<Done>,
}

impl Worker {
    pub fn spawn(renderer: Arc<Renderer>) -> Self {
        let (jobs, job_rx) = mpsc::channel::<Job>();
        let (done_tx, results) = mpsc::channel::<Done>();

        thread::spawn(move || {
            while let Ok(mut job) = job_rx.recv() {
                while let Ok(newer) = job_rx.try_recv() {
                    job = newer;
                }
                let done = Done {
                    generation: job.generation,
                    result: render(&renderer, &job),
                };
                if done_tx.send(done).is_err() {
                    break;
                }
            }
        });

        Self { jobs, results }
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
}

fn render(renderer: &Renderer, job: &Job) -> Result<image::RgbaImage, String> {
    let mut request = RenderRequest::new(&job.source, &job.config);
    if let Some(path) = &job.path {
        request = request.with_path(path.clone());
    }
    let raster = renderer
        .render_raster(&request)
        .map_err(|error| format!("{error:#}"))?;
    image::RgbaImage::from_raw(raster.width, raster.height, raster.pixels)
        .ok_or_else(|| "the preview image was the wrong size".to_string())
}
