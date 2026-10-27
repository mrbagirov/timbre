//! The audio engine: capture, the chain from timbre-dsp, playback into the virtual cable, and meters for the window.
use crate::devices::{self, name_of, Choice};
use crate::UiState;
use arc_swap::ArcSwap;
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, SupportedStreamConfig};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::HeapRb;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use timbre_dsp::resample::Resampler;
use timbre_dsp::{Chain, Flags, FrameMeters, FRAME, SAMPLE_RATE};

pub struct EngineHandle {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl EngineHandle {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FrameOut {
    pre: f32,
    raw: f32,
    gate: f32,
    nr: f32,
    ch: f32,
    comp: f32,
    lim: f32,
    out: f32,
    gate_open: bool,
    nr_db: f32,
    gr_db: f32,
    lim_db: f32,
    loud: f32,
    bands: [f32; 3],
}

#[derive(Serialize, Clone)]
struct Status {
    state: &'static str,
    message: String,
    input: String,
    output: String,
    input_rate: u32,
    output_rate: u32,
}

fn status(
    app: &AppHandle,
    state: &'static str,
    message: impl Into<String>,
    input: &str,
    output: &str,
    ir: u32,
    or: u32,
) {
    let _ = app.emit(
        "engine-status",
        Status {
            state,
            message: message.into(),
            input: input.into(),
            output: output.into(),
            input_rate: ir,
            output_rate: or,
        },
    );
}

/// Prefers 48 kHz in a float or 16-bit format; otherwise the device default.
fn pick_config(
    ranges: Vec<cpal::SupportedStreamConfigRange>,
    default: SupportedStreamConfig,
) -> SupportedStreamConfig {
    let rank = |f: SampleFormat| match f {
        SampleFormat::F32 => 0,
        SampleFormat::I16 => 1,
        SampleFormat::I32 => 2,
        _ => 9,
    };
    let mut best: Option<cpal::SupportedStreamConfigRange> = None;
    for r in ranges {
        if r.min_sample_rate() <= SAMPLE_RATE
            && SAMPLE_RATE <= r.max_sample_rate()
            && rank(r.sample_format()) < 9
        {
            let better = match &best {
                None => true,
                Some(b) => {
                    (rank(r.sample_format()), r.channels())
                        < (rank(b.sample_format()), b.channels())
                }
            };
            if better {
                best = Some(r);
            }
        }
    }
    match best {
        Some(r) => r.with_sample_rate(SAMPLE_RATE),
        None => default,
    }
}

fn build_input<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut prod: ringbuf::HeapProd<f32>,
    errors: Arc<AtomicBool>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    let ch = config.channels as usize;
    let mut rs = Resampler::new(config.sample_rate, SAMPLE_RATE);
    let mut mono: Vec<f32> = Vec::with_capacity(4096);
    let mut conv: Vec<f32> = Vec::with_capacity(4096);
    device.build_input_stream::<T, _, _>(
        config,
        move |data: &[T], _| {
            mono.clear();
            for frame in data.chunks(ch.max(1)) {
                let s: f32 =
                    frame.iter().map(|v| f32::from_sample(*v)).sum::<f32>() / frame.len() as f32;
                mono.push(s);
            }
            conv.clear();
            rs.push(&mono, &mut conv);
            let _ = prod.push_slice(&conv);
        },
        move |_e| errors.store(true, Ordering::SeqCst),
        None,
    )
}

fn build_output<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut cons: ringbuf::HeapCons<f32>,
    errors: Arc<AtomicBool>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample + FromSample<f32> + Send + 'static,
{
    let ch = config.channels as usize;
    let mut rs = Resampler::new(SAMPLE_RATE, config.sample_rate);
    let mut pending: std::collections::VecDeque<f32> =
        std::collections::VecDeque::with_capacity(8192);
    let mut chunk = [0.0f32; FRAME];
    let mut conv: Vec<f32> = Vec::with_capacity(2048);
    device.build_output_stream::<T, _, _>(
        config,
        move |data: &mut [T], _| {
            let frames = data.len() / ch.max(1);
            while pending.len() < frames {
                let n = cons.pop_slice(&mut chunk);
                if n == 0 {
                    break;
                }
                conv.clear();
                rs.push(&chunk[..n], &mut conv);
                pending.extend(conv.iter().copied());
            }
            for frame in data.chunks_mut(ch.max(1)) {
                let v = pending.pop_front().unwrap_or(0.0);
                let t = T::from_sample(v);
                for s in frame.iter_mut() {
                    *s = t;
                }
            }
        },
        move |_e| errors.store(true, Ordering::SeqCst),
        None,
    )
}

fn merge(a: &FrameMeters, b: &FrameMeters) -> FrameMeters {
    FrameMeters {
        pre: a.pre.max(b.pre),
        raw: a.raw.max(b.raw),
        gate: a.gate.max(b.gate),
        nr: a.nr.max(b.nr),
        ch: a.ch.max(b.ch),
        comp: a.comp.max(b.comp),
        lim: a.lim.max(b.lim),
        gate_open: b.gate_open,
        nr_reduction: b.nr_reduction,
        gr: a.gr.max(b.gr),
        lim_cut: a.lim_cut.max(b.lim_cut),
        loud: b.loud,
        bands: [
            (a.bands[0] + b.bands[0]) / 2.0,
            (a.bands[1] + b.bands[1]) / 2.0,
            (a.bands[2] + b.bands[2]) / 2.0,
        ],
    }
}

pub fn start(app: AppHandle, shared: Arc<ArcSwap<UiState>>, choice: Choice) -> EngineHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let join = thread::Builder::new()
        .name("timbre-engine".into())
        .spawn(move || run(app, shared, choice, stop2))
        .expect("engine thread");
    EngineHandle {
        stop,
        join: Some(join),
    }
}

fn run(app: AppHandle, shared: Arc<ArcSwap<UiState>>, choice: Choice, stop: Arc<AtomicBool>) {
    let Some(in_dev) = devices::pick_input(&choice) else {
        status(&app, "error", "Не найден ни один микрофон.", "", "", 0, 0);
        return;
    };
    let in_name = name_of(&in_dev);
    let out_dev = devices::pick_output(&choice);
    let out_name = out_dev.as_ref().map(name_of).unwrap_or_default();

    let errors = Arc::new(AtomicBool::new(false));

    let in_cfg = match in_dev
        .supported_input_configs()
        .ok()
        .zip(in_dev.default_input_config().ok())
    {
        Some((ranges, def)) => pick_config(ranges.collect(), def),
        None => {
            status(
                &app,
                "error",
                format!("Не удалось прочитать форматы микрофона «{in_name}»."),
                &in_name,
                &out_name,
                0,
                0,
            );
            return;
        }
    };
    let (in_prod, mut in_cons) = HeapRb::<f32>::new(SAMPLE_RATE as usize / 2).split();
    let in_stream = {
        let cfg = in_cfg.config();
        let r = match in_cfg.sample_format() {
            SampleFormat::F32 => build_input::<f32>(&in_dev, cfg, in_prod, errors.clone()),
            SampleFormat::I16 => build_input::<i16>(&in_dev, cfg, in_prod, errors.clone()),
            SampleFormat::I32 => build_input::<i32>(&in_dev, cfg, in_prod, errors.clone()),
            other => {
                status(
                    &app,
                    "error",
                    format!("Формат звука микрофона {other:?} пока не поддерживается."),
                    &in_name,
                    &out_name,
                    in_cfg.sample_rate(),
                    0,
                );
                return;
            }
        };
        match r {
            Ok(s) => s,
            Err(e) => {
                status(
                    &app,
                    "error",
                    format!("Не удалось открыть микрофон «{in_name}»: {e}"),
                    &in_name,
                    &out_name,
                    in_cfg.sample_rate(),
                    0,
                );
                return;
            }
        }
    };

    let (mut out_prod, out_cons) = HeapRb::<f32>::new(SAMPLE_RATE as usize / 2).split();
    let mut out_rate = 0;
    let mut out_stream = None;
    let mut out_cons = Some(out_cons);
    if let Some(dev) = &out_dev {
        match (
            dev.supported_output_configs().ok(),
            dev.default_output_config().ok(),
        ) {
            (Some(ranges), Some(def)) => {
                let cfg = pick_config(ranges.collect(), def);
                out_rate = cfg.sample_rate();
                let c = cfg.config();
                let cons = out_cons.take().expect("consumer");
                let r = match cfg.sample_format() {
                    SampleFormat::F32 => build_output::<f32>(dev, c, cons, errors.clone()),
                    SampleFormat::I16 => build_output::<i16>(dev, c, cons, errors.clone()),
                    SampleFormat::I32 => build_output::<i32>(dev, c, cons, errors.clone()),
                    _ => Err(cpal::Error::from(cpal::ErrorKind::UnsupportedConfig)),
                };
                match r {
                    Ok(s) => out_stream = Some(s),
                    Err(e) => status(
                        &app,
                        "error",
                        format!("Не удалось открыть вывод «{out_name}»: {e}"),
                        &in_name,
                        &out_name,
                        in_cfg.sample_rate(),
                        out_rate,
                    ),
                }
            }
            _ => status(
                &app,
                "error",
                format!("Не удалось прочитать форматы вывода «{out_name}»."),
                &in_name,
                &out_name,
                in_cfg.sample_rate(),
                0,
            ),
        }
    }

    if let Err(e) = in_stream.play() {
        status(
            &app,
            "error",
            format!("Микрофон не запускается: {e}"),
            &in_name,
            &out_name,
            in_cfg.sample_rate(),
            out_rate,
        );
        return;
    }
    if let Some(s) = &out_stream {
        let _ = s.play();
    }
    let msg = if out_stream.is_some() {
        "Работает."
    } else {
        "Работает без вывода: виртуальный кабель не найден."
    };
    status(
        &app,
        "running",
        msg,
        &in_name,
        &out_name,
        in_cfg.sample_rate(),
        out_rate,
    );

    let mut chain = Chain::new();
    let mut inbuf = [0.0f32; FRAME];
    let mut out = [0.0f32; FRAME];
    let mut pending: Option<FrameMeters> = None;
    let mut reported_error = false;

    while !stop.load(Ordering::SeqCst) {
        if errors.load(Ordering::SeqCst) && !reported_error {
            reported_error = true;
            status(
                &app,
                "error",
                "Поток звука прервался, возможно, устройство отключено. Нажмите «проверить снова».",
                &in_name,
                &out_name,
                in_cfg.sample_rate(),
                out_rate,
            );
        }
        if in_cons.occupied_len() < FRAME {
            thread::sleep(Duration::from_millis(1));
            continue;
        }
        in_cons.pop_slice(&mut inbuf);
        let st = shared.load();
        let flags = Flags { bypass: st.bypass };
        let m = chain.process(&inbuf, &mut out, &st.params, &flags);
        if st.muted {
            out.fill(0.0);
        }
        if out_stream.is_some() && out_prod.occupied_len() < FRAME * 6 {
            let _ = out_prod.push_slice(&out);
        }
        match pending.take() {
            None => pending = Some(m),
            Some(a) => {
                let c = merge(&a, &m);
                let frame = FrameOut {
                    pre: c.pre,
                    raw: c.raw,
                    gate: c.gate,
                    nr: c.nr,
                    ch: c.ch,
                    comp: c.comp,
                    lim: c.lim,
                    out: if st.muted { -90.0 } else { c.lim },
                    gate_open: c.gate_open,
                    nr_db: c.nr_reduction,
                    gr_db: c.gr,
                    lim_db: c.lim_cut,
                    loud: c.loud,
                    bands: c.bands,
                };
                let _ = app.emit("engine-frame", frame);
            }
        }
    }
    drop(in_stream);
    drop(out_stream);
}

#[allow(dead_code)]
fn _assert_sample<T: Sample>() {}
