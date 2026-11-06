//! Frame distribution and per-device workers.
//!
//! The engine runs one clock. Each device gets a worker whose mailbox holds
//! only the newest frame. A slow device drops intermediate frames instead of
//! building a backlog, which is the difference between "a bit choppy" and
//! "the wave arrives four seconds late".
//!
//! Frames carry an absolute `Instant` target. Each worker learns its own write
//! latency and starts the write half a latency early, so the midpoint of the
//! USB/ACPI transfer lands on the shared target. The numbers are in the README
//! statistics output, not hidden magic.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use log::{debug, error, info, warn};

use crate::color::Rgb;
use crate::device::Device;
use crate::geometry::LedSpec;
use crate::stage::{Stage, StageSample};

/// A frame that should become visible at `target`.
#[derive(Clone, Debug)]
pub struct ScheduledFrame {
    pub seq: u64,
    pub target: Instant,
    pub colors: Vec<Rgb>,
}

struct Mailbox {
    frame: Option<ScheduledFrame>,
    closed: bool,
}

/// Single-slot mailbox. Publishing overwrites an unwritten frame on purpose.
pub struct LatestFrame {
    inner: Mutex<Mailbox>,
    ready: Condvar,
}

impl LatestFrame {
    fn new() -> Self {
        Self {
            inner: Mutex::new(Mailbox {
                frame: None,
                closed: false,
            }),
            ready: Condvar::new(),
        }
    }

    pub fn publish(&self, frame: ScheduledFrame) {
        let mut inner = lock(&self.inner);
        if inner.closed {
            return;
        }
        inner.frame = Some(frame);
        self.ready.notify_one();
    }

    fn close(&self) {
        let mut inner = lock(&self.inner);
        inner.closed = true;
        inner.frame = None;
        self.ready.notify_all();
    }

    fn take(&self, stop: &AtomicBool) -> Option<ScheduledFrame> {
        let mut inner = lock(&self.inner);
        loop {
            if stop.load(Ordering::Relaxed) || inner.closed {
                return None;
            }
            if let Some(frame) = inner.frame.take() {
                return Some(frame);
            }
            let (next, _) = self
                .ready
                .wait_timeout(inner, Duration::from_millis(100))
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            inner = next;
        }
    }
}

/// Counters printed by the run loop.
#[derive(Clone, Debug, Default)]
pub struct DeviceStats {
    pub frames: u64,
    pub dropped: u64,
    pub late: u64,
    pub errors: u64,
    /// Exponentially weighted write duration in microseconds.
    pub latency_us: u64,
}

impl DeviceStats {
    pub fn latency_ms(&self) -> f64 {
        self.latency_us as f64 / 1000.0
    }
}

#[derive(Clone, Debug)]
pub enum WorkerEvent {
    Fatal {
        id: String,
        error: String,
        panicked: bool,
    },
    Recovered {
        id: String,
    },
}

pub struct Worker {
    pub id: String,
    pub kind: &'static str,
    pub layout: Vec<LedSpec>,
    pub stage: Vec<StageSample>,
    latest: Arc<LatestFrame>,
    stats: Arc<Mutex<DeviceStats>>,
    handle: Option<JoinHandle<WorkerExit>>,
}

impl Worker {
    pub fn publish(&self, seq: u64, target: Instant, colors: Vec<Rgb>) {
        self.latest.publish(ScheduledFrame {
            seq,
            target,
            colors,
        });
    }

    pub fn stats(&self) -> DeviceStats {
        lock(&self.stats).clone()
    }
}

struct WorkerExit {
    id: String,
    order: usize,
    device: Box<dyn Device>,
    panicked: bool,
}

/// Owns the workers and performs restore during shutdown, including on panic.
pub struct Session {
    stop: Arc<AtomicBool>,
    workers: Vec<Worker>,
    events: Receiver<WorkerEvent>,
    alive: Arc<AtomicUsize>,
    no_restore: bool,
    shutdown_done: bool,
}

impl Session {
    /// `devices` must already be opened. Order is preserved for restore.
    pub fn start(
        devices: Vec<(Box<dyn Device>, u32)>,
        stage: &Stage,
        stop: Arc<AtomicBool>,
        no_restore: bool,
    ) -> Result<Self> {
        if devices.is_empty() {
            bail!("no devices were opened");
        }
        let (event_tx, event_rx) = mpsc::channel();
        let alive = Arc::new(AtomicUsize::new(devices.len()));
        let mut workers: Vec<Worker> = Vec::with_capacity(devices.len());

        for (order, (device, max_fps)) in devices.into_iter().enumerate() {
            let id = device.id().to_string();
            let kind = device.kind();
            let layout = device.layout().to_vec();
            let stage_samples = stage.samples_for(&id, kind, &layout)?;
            let latest = Arc::new(LatestFrame::new());
            let stats = Arc::new(Mutex::new(DeviceStats::default()));

            let thread_latest = Arc::clone(&latest);
            let thread_stats = Arc::clone(&stats);
            let thread_stop = Arc::clone(&stop);
            let thread_alive = Arc::clone(&alive);
            let thread_tx = event_tx.clone();
            let event_id = id.clone();
            let thread_name = format!("luma-{id}");

            let spawned = thread::Builder::new().name(thread_name).spawn(move || {
                let mut device = device;
                let result = catch_unwind(AssertUnwindSafe(|| {
                    worker_loop(
                        &mut *device,
                        &thread_latest,
                        max_fps,
                        &thread_stop,
                        &thread_stats,
                        &thread_tx,
                    )
                }));
                let (panicked, fatal) = match result {
                    Ok(Ok(())) => (false, None),
                    Ok(Err(err)) => (false, Some(format!("{err:#}"))),
                    Err(_) => (true, Some("worker thread panicked".to_string())),
                };
                if let Some(error) = &fatal {
                    let _ = thread_tx.send(WorkerEvent::Fatal {
                        id: event_id.clone(),
                        error: error.clone(),
                        panicked,
                    });
                }
                thread_alive.fetch_sub(1, Ordering::SeqCst);
                WorkerExit {
                    id: event_id,
                    order,
                    device,
                    panicked,
                }
            });
            let handle = match spawned {
                Ok(handle) => handle,
                Err(err) => {
                    // Keep the already-running workers from leaking. The
                    // device for the failed spawn was dropped inside the
                    // closure, so its restore cannot run here; resource
                    // exhaustion at this point is already a process-level
                    // problem, but at least the other devices are restored.
                    stop.store(true, Ordering::SeqCst);
                    for worker in &workers {
                        worker.latest.close();
                    }
                    for worker in &mut workers {
                        if let Some(handle) = worker.handle.take() {
                            let _ = handle.join();
                        }
                    }
                    return Err(err)
                        .with_context(|| format!("cannot spawn worker for device {id}"));
                }
            };

            workers.push(Worker {
                id,
                kind,
                layout,
                stage: stage_samples,
                latest,
                stats,
                handle: Some(handle),
            });
        }

        info!(
            "started {} device workers: {}",
            workers.len(),
            workers
                .iter()
                .map(|w| w.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        Ok(Self {
            stop,
            workers,
            events: event_rx,
            alive,
            no_restore,
            shutdown_done: false,
        })
    }

    pub fn workers(&self) -> &[Worker] {
        &self.workers
    }

    pub fn poll_events(&self) -> Vec<WorkerEvent> {
        self.events.try_iter().collect()
    }

    pub fn alive_workers(&self) -> usize {
        self.alive.load(Ordering::SeqCst)
    }

    pub fn stats(&self) -> Vec<(String, DeviceStats)> {
        self.workers
            .iter()
            .map(|worker| (worker.id.clone(), worker.stats()))
            .collect()
    }

    /// Stop every worker, collect the devices, and restore them in startup
    /// order. Safe to call more than once.
    pub fn shutdown(&mut self) -> Result<()> {
        if self.shutdown_done {
            return Ok(());
        }
        self.shutdown_done = true;
        self.stop.store(true, Ordering::SeqCst);
        for worker in &self.workers {
            worker.latest.close();
        }

        let mut exits = Vec::new();
        let mut errors = Vec::new();
        for worker in &mut self.workers {
            if let Some(handle) = worker.handle.take() {
                match handle.join() {
                    Ok(exit) => {
                        if exit.panicked {
                            errors.push(format!("{}: worker panicked", exit.id));
                        }
                        exits.push(exit);
                    }
                    Err(_) => errors.push(format!("{}: worker join failed", worker.id)),
                }
            }
        }
        exits.sort_by_key(|exit| exit.order);

        if self.no_restore {
            warn!("restore disabled by --no-restore; leaving the last frame on the devices");
            for mut exit in exits {
                exit.device.close();
            }
        } else {
            for mut exit in exits {
                match exit.device.restore() {
                    Ok(()) => debug!("{}: restore complete", exit.id),
                    Err(err) => {
                        error!("{}: restore failed: {err:#}", exit.id);
                        errors.push(format!("{}: restore failed: {err:#}", exit.id));
                    }
                }
                exit.device.close();
            }
        }

        if errors.is_empty() {
            info!("all devices restored");
            Ok(())
        } else {
            bail!("shutdown problems: {}", errors.join("; "))
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Err(err) = self.shutdown() {
            error!("{err:#}");
        }
    }
}

fn worker_loop(
    device: &mut dyn Device,
    latest: &LatestFrame,
    max_fps: u32,
    stop: &AtomicBool,
    stats: &Mutex<DeviceStats>,
    events: &Sender<WorkerEvent>,
) -> Result<()> {
    let min_interval = Duration::from_secs_f64(1.0 / f64::from(max_fps.max(1)));
    let mut last_target: Option<Instant> = None;
    let mut ewma_us: Option<u64> = None;

    loop {
        let Some(frame) = latest.take(stop) else {
            return Ok(());
        };
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }

        if let Some(previous) = last_target {
            let delta = frame.target.saturating_duration_since(previous);
            if delta + Duration::from_millis(2) < min_interval {
                update_stats(stats, |s| s.dropped += 1);
                continue;
            }
        }

        let now = Instant::now();
        if frame.target + Duration::from_millis(100) < now {
            debug!("{}: dropping stale frame {}", device.id(), frame.seq);
            update_stats(stats, |s| s.dropped += 1);
            continue;
        }

        // Aim the middle of the transfer at the target. Clamp the lead so a
        // pathological latency measurement cannot make us sleep in the past
        // for an entire frame period.
        let lead = Duration::from_micros(ewma_us.unwrap_or(0) / 2)
            .min(Duration::from_millis(20))
            .min(min_interval / 2);
        if let Some(wake) = frame.target.checked_sub(lead) {
            sleep_until(wake, stop);
        }
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }

        let started = Instant::now();
        match device.send(&frame.colors) {
            Ok(()) => {
                let elapsed = started.elapsed();
                update_stats(stats, |s| {
                    s.frames += 1;
                    if started > frame.target + Duration::from_millis(2) {
                        s.late += 1;
                    }
                    let sample = elapsed.as_micros() as u64;
                    s.latency_us = if s.latency_us == 0 {
                        sample
                    } else {
                        (s.latency_us as f64 * 0.8 + sample as f64 * 0.2) as u64
                    };
                });
                ewma_us = Some(match ewma_us {
                    None => elapsed.as_micros() as u64,
                    Some(old) => (old as f64 * 0.8 + elapsed.as_micros() as f64 * 0.2) as u64,
                });
                last_target = Some(frame.target);
            }
            Err(err) => {
                update_stats(stats, |s| s.errors += 1);
                warn!("{}: frame write failed: {err:#}", device.id());
                device.close();
                match reconnect(device, stop, events)? {
                    true => {}
                    false => return Ok(()),
                }
            }
        }
    }
}

/// Try to reopen a lost device with bounded exponential backoff.
/// Returns Ok(true) when reconnected, Ok(false) when shutdown was requested.
fn reconnect(
    device: &mut dyn Device,
    stop: &AtomicBool,
    events: &Sender<WorkerEvent>,
) -> Result<bool> {
    let mut delay = Duration::from_millis(250);
    for attempt in 1..=6 {
        if stop.load(Ordering::Relaxed) {
            return Ok(false);
        }
        sleep_until(Instant::now() + delay, stop);
        if stop.load(Ordering::Relaxed) {
            return Ok(false);
        }
        match device.open() {
            Ok(()) => {
                info!("{}: reconnected after {attempt} attempt(s)", device.id());
                let _ = events.send(WorkerEvent::Recovered {
                    id: device.id().to_string(),
                });
                return Ok(true);
            }
            Err(err) => warn!(
                "{}: reconnect attempt {attempt}/6 failed: {err:#}",
                device.id()
            ),
        }
        delay = (delay * 2).min(Duration::from_secs(4));
    }
    bail!("{}: reconnect failed after 6 attempts", device.id())
}

pub(crate) fn sleep_until(target: Instant, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        if now >= target {
            return;
        }
        thread::sleep((target - now).min(Duration::from_millis(100)));
    }
}

fn update_stats(stats: &Mutex<DeviceStats>, update: impl FnOnce(&mut DeviceStats)) {
    let mut stats = lock(stats);
    update(&mut stats);
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mailbox_keeps_only_latest_frame() {
        let mailbox = LatestFrame::new();
        let stop = AtomicBool::new(false);
        let target = Instant::now();
        mailbox.publish(ScheduledFrame {
            seq: 1,
            target,
            colors: vec![Rgb::new(1, 0, 0)],
        });
        mailbox.publish(ScheduledFrame {
            seq: 2,
            target,
            colors: vec![Rgb::new(2, 0, 0)],
        });
        let frame = mailbox.take(&stop).unwrap();
        assert_eq!(frame.seq, 2);
    }

    #[test]
    fn close_wakes_take() {
        let mailbox = Arc::new(LatestFrame::new());
        let stop = Arc::new(AtomicBool::new(false));
        let waiter = {
            let mailbox = Arc::clone(&mailbox);
            let stop = Arc::clone(&stop);
            thread::spawn(move || mailbox.take(&stop))
        };
        thread::sleep(Duration::from_millis(20));
        mailbox.close();
        assert!(waiter.join().unwrap().is_none());
    }

    #[test]
    fn stats_latency_is_milliseconds() {
        let stats = DeviceStats {
            latency_us: 2500,
            ..Default::default()
        };
        assert!((stats.latency_ms() - 2.5).abs() < f64::EPSILON);
    }
}
