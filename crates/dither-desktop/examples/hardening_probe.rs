use std::{
    env,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use dither_desktop::components::processing::{
    Dispatch, DocumentId, ProcessingCoordinator, ProcessingEvent, RenderRequest, Revision,
};
use dither_engine::{
    AtomicCancellationToken, EffectConfig, NeverCancel, NoProgress, Raster, RenderError, render,
};

const WIDTH: u32 = 3_840;
const HEIGHT: u32 = 2_160;

fn main() -> Result<(), String> {
    match env::args().nth(1).as_deref() {
        Some("memory") => memory_probe(),
        Some("cancellation") => cancellation_probe(),
        Some("soak") => soak_probe(),
        _ => Err("uso: hardening_probe <memory|cancellation|soak>".to_owned()),
    }
}

fn large_raster() -> Result<Raster, String> {
    let pixels = u64::from(WIDTH) * u64::from(HEIGHT);
    let bytes = usize::try_from(pixels * 4).map_err(|error| error.to_string())?;
    let mut rgba = vec![0_u8; bytes];
    for (index, pixel) in rgba.chunks_exact_mut(4).enumerate() {
        let value = u8::try_from(index % 256).map_err(|error| error.to_string())?;
        pixel.copy_from_slice(&[value, value, value, 255]);
    }
    Raster::new(WIDTH, HEIGHT, rgba).map_err(|error| error.to_string())
}

fn memory_probe() -> Result<(), String> {
    let source = large_raster()?;
    let started = Instant::now();
    let result = render(&source, &EffectConfig::default(), &NeverCancel, &NoProgress)
        .map_err(|error| error.to_string())?;
    let checksum: u64 = result.rgba().iter().map(|value| u64::from(*value)).sum();
    println!(
        "mode=memory pixels={} elapsed_ms={} checksum={checksum}",
        u64::from(WIDTH) * u64::from(HEIGHT),
        started.elapsed().as_millis()
    );
    thread::sleep(Duration::from_secs(2));
    Ok(())
}

fn cancellation_probe() -> Result<(), String> {
    let source = large_raster()?;
    let token = AtomicCancellationToken::new();
    let canceller = token.clone();
    let signal = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        let instant = Instant::now();
        canceller.cancel();
        instant
    });
    let result = render(&source, &EffectConfig::default(), &token, &NoProgress);
    let finished = Instant::now();
    let cancelled_at = signal
        .join()
        .map_err(|_| "falló el hilo de cancelación".to_owned())?;
    if !matches!(result, Err(RenderError::Cancelled)) {
        return Err("el render no terminó como cancelado".to_owned());
    }
    println!(
        "mode=cancellation response_ms={}",
        finished.duration_since(cancelled_at).as_millis()
    );
    Ok(())
}

fn soak_probe() -> Result<(), String> {
    let source = Arc::new(
        Raster::new(512, 512, vec![127; 512 * 512 * 4]).map_err(|error| error.to_string())?,
    );
    let mut coordinator = ProcessingCoordinator::new().map_err(|error| error.to_string())?;
    let started = Instant::now();
    let duration = Duration::from_secs(30);
    let mut revision = 0_u64;
    while started.elapsed() < duration {
        for _ in 0..50 {
            revision = revision.saturating_add(1);
            coordinator.submit(
                RenderRequest::new(
                    DocumentId::new(1),
                    Revision::new(revision),
                    Arc::clone(&source),
                    EffectConfig::default(),
                ),
                Dispatch::Immediate,
            );
        }
        while let Some(_event) = coordinator.poll_event() {}
        thread::sleep(Duration::from_millis(5));
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if matches!(
            coordinator.poll_event(),
            Some(ProcessingEvent::Completed { revision: value, .. }) if value == Revision::new(revision)
        ) {
            break;
        }
        if Instant::now() >= deadline {
            return Err("el último render no terminó tras la prueba larga".to_owned());
        }
        thread::sleep(Duration::from_millis(5));
    }
    if !coordinator.shutdown(Duration::from_secs(1)) {
        return Err("el trabajador no cerró dentro del límite".to_owned());
    }
    println!(
        "mode=soak revisions={revision} elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(())
}
