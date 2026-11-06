//! The central frame loop and latency statistics.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use log::{info, warn};

use crate::config::ErrorPolicy;
use crate::engine::Engine;
use crate::sync::{Session, WorkerEvent, sleep_until};

pub(crate) fn drive(
    session: &Session,
    engine: &Engine,
    fps: u32,
    duration: Option<f64>,
    stats_interval_secs: f64,
    on_error: ErrorPolicy,
    stop: &AtomicBool,
) -> Result<()> {
    let start = Instant::now();
    let frame_period = Duration::from_secs_f64(1.0 / f64::from(fps));
    let stats_interval = Duration::from_secs_f64(stats_interval_secs.max(0.0));
    let mut next_stats = start + stats_interval;
    let mut seq: u64 = 0;
    let mut jumped = false;

    info!(
        "running {} at {} fps; press Ctrl-C to stop",
        engine.name(),
        fps
    );

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if let Some(limit) = duration {
            if seq as f64 / f64::from(fps) >= limit {
                break;
            }
        }

        let target = start + Duration::from_secs_f64(seq as f64 / f64::from(fps));
        if target > Instant::now() {
            sleep_until(target, stop);
        }
        if stop.load(Ordering::Relaxed) {
            break;
        }

        if Instant::now().saturating_duration_since(target) > frame_period * 2 {
            let skip_to = (start.elapsed().as_secs_f64() * f64::from(fps)).ceil() as u64;
            if skip_to > seq {
                if !jumped {
                    warn!(
                        "render loop fell behind; skipping frames {seq}..={skip_to}; \
                         lower --fps if this repeats"
                    );
                    jumped = true;
                }
                seq = skip_to;
                continue;
            }
        }

        let time = seq as f64 / f64::from(fps);
        for worker in session.workers() {
            let colors = engine.render(time, &worker.layout);
            worker.publish(seq, target, colors);
        }

        for event in session.poll_events() {
            match event {
                WorkerEvent::Fatal {
                    id,
                    error,
                    panicked,
                } => {
                    if panicked {
                        bail!("device {id}: worker panicked: {error}");
                    }
                    match on_error {
                        ErrorPolicy::Stop => bail!("device {id}: {error}"),
                        ErrorPolicy::Continue => {
                            warn!("device {id}: {error}; continuing with the remaining devices")
                        }
                    }
                }
                WorkerEvent::Recovered { id } => info!("device {id}: recovered"),
            }
        }
        if session.alive_workers() == 0 {
            bail!("all device workers stopped");
        }

        if stats_interval_secs > 0.0 && Instant::now() >= next_stats {
            info!("{}", format_stats(session));
            let now = Instant::now();
            while next_stats <= now {
                next_stats += stats_interval;
            }
        }

        seq += 1;
    }

    info!("stopping: {}", format_stats(session));
    Ok(())
}

fn format_stats(session: &Session) -> String {
    let stats = session
        .stats()
        .into_iter()
        .map(|(id, stats)| {
            format!(
                "{id}: {}f {:.1}ms late={} drop={} err={}",
                stats.frames,
                stats.latency_ms(),
                stats.late,
                stats.dropped,
                stats.errors
            )
        })
        .collect::<Vec<_>>();
    format!("sync stats: {}", stats.join(" | "))
}
