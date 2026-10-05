use std::fs::File;
use std::path::Path;

use symphonia::core::audio::{SampleBuffer, SignalSpec};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::AnalysisError;

/// A fully decoded track, downmixed to mono `f32` samples in `[-1.0, 1.0]`.
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// Decodes the entire file into memory as mono f32 PCM. Tracks are a few
/// minutes long, so this is a bounded, acceptable amount of memory for a
/// single background-worker job (a 5-minute track at 44.1kHz is ~50MB as
/// f32 mono) — not something we'd do for a batch held in memory at once.
pub fn decode_to_mono(path: &Path) -> Result<DecodedAudio, AnalysisError> {
    let file = File::open(path).map_err(|e| AnalysisError::Io(e.to_string()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| AnalysisError::Decode(e.to_string()))?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or_else(|| AnalysisError::Decode("no playable track found".into()))?
        .clone();

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| AnalysisError::Decode(e.to_string()))?;

    let track_id = track.id;
    let mut samples: Vec<f32> = Vec::new();
    let mut sample_rate: u32 = track.codec_params.sample_rate.unwrap_or(44100);
    let mut channels: usize = track
        .codec_params
        .channels
        .map(|c| c.count())
        .unwrap_or(1)
        .max(1);
    let mut sample_buf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(AnalysisError::Decode(e.to_string())),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                if sample_buf.is_none() {
                    let spec: SignalSpec = *decoded.spec();
                    sample_rate = spec.rate;
                    channels = spec.channels.count().max(1);
                    sample_buf = Some(SampleBuffer::new(decoded.capacity() as u64, spec));
                }
                let buf = sample_buf.as_mut().unwrap();
                buf.copy_interleaved_ref(decoded);
                downmix_into(buf.samples(), channels, &mut samples);
            }
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(AnalysisError::Decode(e.to_string())),
        }
    }

    if samples.is_empty() {
        return Err(AnalysisError::Decode("no audio samples decoded".into()));
    }

    Ok(DecodedAudio { samples, sample_rate })
}

fn downmix_into(interleaved: &[f32], channels: usize, out: &mut Vec<f32>) {
    if channels <= 1 {
        out.extend_from_slice(interleaved);
        return;
    }
    out.reserve(interleaved.len() / channels);
    for frame in interleaved.chunks_exact(channels) {
        let sum: f32 = frame.iter().sum();
        out.push(sum / channels as f32);
    }
}
