use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct AudioReply {
    pub samples: Vec<f32>,
    pub rate: u32,
    pub channels: u16,
}

pub enum AudioCmd {
    Start { device: Option<String> },
    Stop,
    Cancel,
}

pub fn list_mics() -> Vec<String> {
    let host = cpal::default_host();
    host.input_devices().map(|it| it.filter_map(|d| d.name().ok()).collect()).unwrap_or_default()
}

pub fn spawn(level: Arc<AtomicU32>) -> (std::sync::mpsc::Sender<AudioCmd>, std::sync::mpsc::Receiver<AudioReply>) {
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
    let (reply_tx, reply_rx) = std::sync::mpsc::channel();
    std::thread::Builder::new().name("mic".into()).spawn(move || audio_loop(cmd_rx, reply_tx, level)).ok();
    (cmd_tx, reply_rx)
}

fn audio_loop(cmd_rx: std::sync::mpsc::Receiver<AudioCmd>, reply_tx: std::sync::mpsc::Sender<AudioReply>, level: Arc<AtomicU32>) {
    let buf = Arc::new(Mutex::new(Vec::<f32>::new()));
    let meta = Arc::new(Mutex::new((16_000u32, 1u16)));
    let mut stream: Option<cpal::Stream> = None;
    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            AudioCmd::Start { device } => {
                buf.lock().unwrap().clear();
                level.store(0, Ordering::Relaxed);
                stream = match start_stream(device.as_deref(), buf.clone(), meta.clone(), level.clone()) {
                    Ok(s) => Some(s),
                    Err(err) => {
                        eprintln!("mic start: {err}");
                        None
                    }
                };
            }
            AudioCmd::Stop | AudioCmd::Cancel => {
                drop(stream.take());
                let samples = std::mem::take(&mut *buf.lock().unwrap());
                let (rate, channels) = *meta.lock().unwrap();
                level.store(0, Ordering::Relaxed);
                if matches!(cmd, AudioCmd::Stop) {
                    let _ = reply_tx.send(AudioReply { samples, rate, channels });
                }
            }
        }
    }
}

fn start_stream(name: Option<&str>, buf: Arc<Mutex<Vec<f32>>>, meta: Arc<Mutex<(u32, u16)>>, level: Arc<AtomicU32>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = if let Some(name) = name {
        host.input_devices().ok().and_then(|mut devices| devices.find(|d| d.name().ok().as_deref() == Some(name)))
    } else {
        None
    };
    let device = device.or_else(|| host.default_input_device()).ok_or("Nincs mikrofon.")?;
    let config = device.default_input_config().map_err(|_| "A mikrofon nem nyitható meg.".to_string())?;
    let channels = config.channels();
    let rate = config.sample_rate().0;
    *meta.lock().unwrap() = (rate, channels);
    let err_fn = |err| eprintln!("mic: {err}");
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let buf = buf.clone();
            let level = level.clone();
            device.build_input_stream(&config.into(), move |data: &[f32], _| push_f32(data, channels, &buf, &level), err_fn, None)
        }
        cpal::SampleFormat::I16 => {
            let buf = buf.clone();
            let level = level.clone();
            device.build_input_stream(&config.into(), move |data: &[i16], _| push_scaled(data, channels, &buf, &level, |s| s as f32 / 32768.0), err_fn, None)
        }
        cpal::SampleFormat::U16 => {
            let buf = buf.clone();
            let level = level.clone();
            device.build_input_stream(&config.into(), move |data: &[u16], _| push_scaled(data, channels, &buf, &level, |s| s as f32 / 32768.0 - 1.0), err_fn, None)
        }
        _ => return Err("A mikrofon formátuma nem támogatott.".into()),
    }
    .map_err(|_| "A mikrofon megnyitása nem sikerült. Ellenőrizd az engedélyt.".to_string())?;
    stream.play().map_err(|_| "A felvétel nem indult el.".to_string())?;
    let _ = Duration::from_millis(1);
    Ok(stream)
}

fn push_f32(data: &[f32], channels: u16, buf: &Mutex<Vec<f32>>, level: &AtomicU32) {
    push_scaled(data, channels, buf, level, |s| s);
}

fn push_scaled<T: Copy>(data: &[T], channels: u16, buf: &Mutex<Vec<f32>>, level: &AtomicU32, map: impl Fn(T) -> f32) {
    let ch = channels.max(1) as usize;
    let mut guard = buf.lock().unwrap();
    guard.extend(data.iter().copied().map(&map));
    if guard.len() > 16_000 * 60 * 4 {
        let drain = guard.len() - 16_000 * 60 * 3;
        guard.drain(0..drain);
    }
    drop(guard);
    if data.is_empty() {
        return;
    }
    let mut acc = 0f32;
    let mut n = 0usize;
    for (i, s) in data.iter().enumerate() {
        if i % ch == 0 {
            let v = map(*s);
            acc += v * v;
            n += 1;
        }
    }
    if n > 0 {
        let rms = (acc / n as f32).sqrt();
        let db = 20.0 * rms.max(1e-6).log10();
        let norm = ((db + 50.0) / 40.0).clamp(0.0, 1.0);
        level.store((norm * 1000.0) as u32, std::sync::atomic::Ordering::Relaxed);
    }
}
