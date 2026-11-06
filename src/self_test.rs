//! Visible test patterns used by `luma self-test`.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::app::{load_config, open_devices, select_devices};
use crate::cli::SelfTestArgs;
use crate::color::Rgb;
use crate::stage::Stage;
use crate::sync::Session;

pub fn self_test(
    config_path: Option<&Path>,
    args: SelfTestArgs,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    let config = load_config(config_path)?;
    let mut selected = select_devices(&config, args.device.as_deref())?;

    if args.keymap && args.keys.is_some() {
        bail!("--keymap and --keys are mutually exclusive");
    }
    if args.keymap || args.keys.is_some() {
        let keyboards: Vec<_> = selected
            .iter()
            .copied()
            .filter(|device| device.kind.trim().eq_ignore_ascii_case("k617"))
            .collect();
        if keyboards.len() != 1 {
            bail!(
                "the key test needs exactly one enabled k617 device in the selection; found {}",
                keyboards.len()
            );
        }
        selected = keyboards;
    }

    let fps = config.general.fps;
    let stage = Stage::new(&config.stage);
    stage.validate_for(
        selected
            .iter()
            .map(|device| (device.id.as_str(), device.kind.as_str())),
    )?;
    let devices = open_devices(&selected, args.dry_run, args.no_restore, fps)?;
    let session = Session::start(devices, &stage, Arc::clone(&stop), args.no_restore)?;

    let test_result = if args.keymap {
        run_keymap_test(&session, &stop)
    } else if let Some(keys) = &args.keys {
        run_keys_test(&session, &stop, keys)
    } else {
        run_color_test(&session, &stop)
    };
    let shutdown_result = {
        let mut session = session;
        session.shutdown()
    };
    match (test_result, shutdown_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(err), Ok(())) => Err(err),
        (Ok(()), Err(err)) => Err(err),
        (Err(err), Err(shutdown)) => {
            Err(err.context(format!("shutdown also failed: {shutdown:#}")))
        }
    }
}

const TEST_FRAME_INTERVAL: Duration = Duration::from_millis(33);

fn run_color_test(session: &Session, stop: &AtomicBool) -> Result<()> {
    let steps = [
        ("red", Rgb::new(255, 0, 0)),
        ("green", Rgb::new(0, 255, 0)),
        ("blue", Rgb::new(0, 0, 255)),
        ("white", Rgb::WHITE),
        ("off", Rgb::BLACK),
    ];
    let mut seq = 0u64;
    for (name, color) in steps {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        println!("self-test: {name} ({})", color.to_hex());
        stream_solid(session, &mut seq, color, Duration::from_millis(600), stop);
    }
    Ok(())
}

fn run_keymap_test(session: &Session, stop: &AtomicBool) -> Result<()> {
    let worker = session
        .workers()
        .iter()
        .find(|worker| worker.kind == "k617")
        .context("no k617 worker in the keymap test")?;
    println!(
        "keymap sweep: watch which physical key lights for each slot ({} keys)",
        worker.layout.len()
    );
    let mut seq = 0u64;
    for (index, led) in worker.layout.iter().enumerate() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        println!(
            "  {index:02}/{} {} (normalized {:.2},{:.2})",
            worker.layout.len() - 1,
            led.name,
            led.position.x,
            led.position.y
        );
        stream_key(worker, index, &mut seq, Duration::from_millis(300), stop);
    }
    stream_solid(
        session,
        &mut seq,
        Rgb::BLACK,
        Duration::from_millis(250),
        stop,
    );
    Ok(())
}

fn run_keys_test(session: &Session, stop: &AtomicBool, names: &str) -> Result<()> {
    let worker = session
        .workers()
        .iter()
        .find(|worker| worker.kind == "k617")
        .context("no k617 worker in the key test")?;
    let available = worker
        .layout
        .iter()
        .map(|led| led.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let mut seq = 0u64;
    for name in names
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let index = worker
            .layout
            .iter()
            .position(|led| led.name.eq_ignore_ascii_case(name))
            .with_context(|| format!("unknown K617 key {name:?}; available: {available}"))?;
        println!(
            "key test: {name} (LED index {index}); background is dim blue, \
             the named key should be bright white"
        );
        stream_key(worker, index, &mut seq, Duration::from_millis(1500), stop);
    }
    stream_solid(
        session,
        &mut seq,
        Rgb::BLACK,
        Duration::from_millis(250),
        stop,
    );
    Ok(())
}

/// Keep re-sending the same frame for a hold period. The K617 leaves direct
/// per-key mode when frames stop arriving, so a single frame is not enough for
/// a visually useful test.
fn stream_solid(
    session: &Session,
    seq: &mut u64,
    color: Rgb,
    duration: Duration,
    stop: &AtomicBool,
) {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline && !stop.load(Ordering::Relaxed) {
        publish_solid(session, *seq, Instant::now(), color);
        *seq += 1;
        thread::sleep(TEST_FRAME_INTERVAL);
    }
}

fn stream_key(
    worker: &crate::sync::Worker,
    index: usize,
    seq: &mut u64,
    duration: Duration,
    stop: &AtomicBool,
) {
    // A completely black background makes the keyboard look disconnected if
    // one slot is wrong. A dim blue field keeps every LED alive and makes the
    // single bright target easy to spot.
    const BACKGROUND: Rgb = Rgb::new(0, 0, 24);
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline && !stop.load(Ordering::Relaxed) {
        let mut colors = vec![BACKGROUND; worker.layout.len()];
        colors[index] = Rgb::WHITE;
        worker.publish(*seq, Instant::now(), colors);
        *seq += 1;
        thread::sleep(TEST_FRAME_INTERVAL);
    }
}

fn publish_solid(session: &Session, seq: u64, target: Instant, color: Rgb) {
    for worker in session.workers() {
        let colors = vec![color; worker.layout.len()];
        worker.publish(seq, target, colors);
    }
}
