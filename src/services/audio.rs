use crate::error::AppError;
use crate::services::synth::FloatAudioBuffer;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AudioBuffer {
    pub pcm_s16le: Vec<u8>,
    pub sample_rate: u32,
    pub channels: u16,
}

pub(crate) fn float_audio_to_pcm(audio: &FloatAudioBuffer) -> Result<AudioBuffer, AppError> {
    if audio.sample_rate == 0 {
        return Err(AppError::validation("sample_rate must be positive"));
    }
    if audio.channels != 1 {
        return Err(AppError::validation(
            "backend float audio must be mono before normalization",
        ));
    }

    let mut pcm_s16le = Vec::with_capacity(audio.waveform.len() * 2);
    for sample in &audio.waveform {
        let clipped = sample.clamp(-1.0, 1.0);
        // Matches Python: KittenTTS multiplies by 32767, not 32768, producing a
        // symmetric range of -32767..=32767. Do not change to 32768.0 — that
        // would break byte-level compatibility with the Python server output.
        let scaled = (clipped * 32767.0).round() as i32;
        let clamped = scaled.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        pcm_s16le.extend_from_slice(&clamped.to_le_bytes());
    }

    Ok(AudioBuffer {
        pcm_s16le,
        sample_rate: audio.sample_rate,
        channels: audio.channels,
    })
}

pub(crate) fn normalize_audio(
    audio: &AudioBuffer,
    sample_rate: u32,
    channels: u16,
) -> Result<AudioBuffer, AppError> {
    validate_audio(audio)?;
    if sample_rate == 0 {
        return Err(AppError::validation("target sample_rate must be positive"));
    }
    if !matches!(channels, 1 | 2) {
        return Err(AppError::validation("target channels must be 1 or 2"));
    }

    let converted_channels = convert_channels(&audio.pcm_s16le, audio.channels, channels)?;
    let normalized_pcm = if audio.sample_rate != sample_rate {
        resample_linear(
            &converted_channels,
            audio.sample_rate,
            sample_rate,
            channels,
        )?
    } else {
        converted_channels
    };

    Ok(AudioBuffer {
        pcm_s16le: normalized_pcm,
        sample_rate,
        channels,
    })
}

pub(crate) fn serialize_wav(audio: &AudioBuffer) -> Result<Vec<u8>, AppError> {
    validate_audio(audio)?;

    let data_len = u32::try_from(audio.pcm_s16le.len())
        .map_err(|_| AppError::internal("PCM payload is too large for WAV serialization"))?;
    let riff_len = 36u32
        .checked_add(data_len)
        .ok_or_else(|| AppError::internal("WAV payload length overflow"))?;
    let byte_rate = audio
        .sample_rate
        .checked_mul(u32::from(audio.channels))
        .and_then(|value| value.checked_mul(2))
        .ok_or_else(|| AppError::internal("WAV byte_rate overflow"))?;
    let block_align = audio
        .channels
        .checked_mul(2)
        .ok_or_else(|| AppError::internal("WAV block_align overflow"))?;

    let mut wav = Vec::with_capacity(44 + audio.pcm_s16le.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_len.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&audio.channels.to_le_bytes());
    wav.extend_from_slice(&audio.sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(&audio.pcm_s16le);

    Ok(wav)
}

pub(crate) fn serialize_pcm_s16le(audio: &AudioBuffer) -> Result<Vec<u8>, AppError> {
    validate_audio(audio)?;
    Ok(audio.pcm_s16le.clone())
}

fn validate_audio(audio: &AudioBuffer) -> Result<(), AppError> {
    if audio.sample_rate == 0 {
        return Err(AppError::validation("sample_rate must be positive"));
    }
    if !matches!(audio.channels, 1 | 2) {
        return Err(AppError::validation("channels must be 1 or 2"));
    }

    let frame_width = usize::from(audio.channels) * 2;
    if !audio.pcm_s16le.len().is_multiple_of(frame_width) {
        return Err(AppError::validation(
            "pcm_s16le length must align to 16-bit sample frames",
        ));
    }

    Ok(())
}

fn convert_channels(
    pcm_s16le: &[u8],
    source_channels: u16,
    target_channels: u16,
) -> Result<Vec<u8>, AppError> {
    if source_channels == target_channels {
        return Ok(pcm_s16le.to_vec());
    }

    let samples = pcm_to_samples(pcm_s16le)?;
    match (source_channels, target_channels) {
        (2, 1) => {
            let mut mono_samples = Vec::with_capacity(samples.len() / 2);
            for frame in samples.chunks_exact(2) {
                let averaged = (i32::from(frame[0]) + i32::from(frame[1])) / 2;
                mono_samples.push(clip_s16(averaged));
            }
            Ok(samples_to_pcm(&mono_samples))
        }
        (1, 2) => {
            let mut stereo_samples = Vec::with_capacity(samples.len() * 2);
            for sample in samples {
                stereo_samples.push(sample);
                stereo_samples.push(sample);
            }
            Ok(samples_to_pcm(&stereo_samples))
        }
        _ => Err(AppError::validation("unsupported channel conversion")),
    }
}

fn resample_linear(
    pcm_s16le: &[u8],
    source_rate: u32,
    target_rate: u32,
    channels: u16,
) -> Result<Vec<u8>, AppError> {
    if source_rate == target_rate || pcm_s16le.is_empty() {
        return Ok(pcm_s16le.to_vec());
    }

    let samples = pcm_to_samples(pcm_s16le)?;
    let channels_usize = usize::from(channels);
    let source_frame_count = samples.len() / channels_usize;
    let target_frame_count = ((source_frame_count as f64 * f64::from(target_rate)
        / f64::from(source_rate))
    .round() as usize)
        .max(1);
    let mut resampled_samples = Vec::with_capacity(target_frame_count * channels_usize);

    for output_frame_index in 0..target_frame_count {
        let source_position =
            output_frame_index as f64 * f64::from(source_rate) / f64::from(target_rate);
        let left_frame_index = source_position.floor() as usize;
        let left_frame_index = left_frame_index.min(source_frame_count - 1);
        let right_frame_index = (left_frame_index + 1).min(source_frame_count - 1);
        let interpolation = source_position - left_frame_index as f64;

        for channel_index in 0..channels_usize {
            let left_sample = samples[(left_frame_index * channels_usize) + channel_index] as f64;
            let right_sample = samples[(right_frame_index * channels_usize) + channel_index] as f64;
            let interpolated =
                (left_sample * (1.0 - interpolation) + right_sample * interpolation).round() as i32;
            resampled_samples.push(clip_s16(interpolated));
        }
    }

    Ok(samples_to_pcm(&resampled_samples))
}

fn clip_s16(sample: i32) -> i16 {
    sample.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

fn pcm_to_samples(pcm_s16le: &[u8]) -> Result<Vec<i16>, AppError> {
    if !pcm_s16le.len().is_multiple_of(2) {
        return Err(AppError::validation(
            "pcm_s16le length must align to 16-bit sample frames",
        ));
    }

    Ok(pcm_s16le
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect())
}

fn samples_to_pcm(samples: &[i16]) -> Vec<u8> {
    let mut pcm = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        pcm.extend_from_slice(&sample.to_le_bytes());
    }
    pcm
}

type FloatAudioToPcmFn = fn(&FloatAudioBuffer) -> Result<AudioBuffer, AppError>;
type NormalizeAudioFn = fn(&AudioBuffer, u32, u16) -> Result<AudioBuffer, AppError>;
type SerializeWavFn = fn(&AudioBuffer) -> Result<Vec<u8>, AppError>;
type SerializePcmFn = fn(&AudioBuffer) -> Result<Vec<u8>, AppError>;
type ValidateAudioFn = fn(&AudioBuffer) -> Result<(), AppError>;
type ConvertChannelsFn = fn(&[u8], u16, u16) -> Result<Vec<u8>, AppError>;
type ResampleLinearFn = fn(&[u8], u32, u32, u16) -> Result<Vec<u8>, AppError>;
type ClipS16Fn = fn(i32) -> i16;
type PcmToSamplesFn = fn(&[u8]) -> Result<Vec<i16>, AppError>;
type SamplesToPcmFn = fn(&[i16]) -> Vec<u8>;

// Compile-time signature checks: these `const _:` bindings verify that each
// crate-private function still matches its declared type alias. They produce
// a type error at compile time if a function signature drifts from the alias,
// which acts as a lightweight contract test without any runtime cost.
const _: Option<AudioBuffer> = None;
const _: FloatAudioToPcmFn = float_audio_to_pcm;
const _: NormalizeAudioFn = normalize_audio;
const _: SerializeWavFn = serialize_wav;
const _: SerializePcmFn = serialize_pcm_s16le;
const _: ValidateAudioFn = validate_audio;
const _: ConvertChannelsFn = convert_channels;
const _: ResampleLinearFn = resample_linear;
const _: ClipS16Fn = clip_s16;
const _: PcmToSamplesFn = pcm_to_samples;
const _: SamplesToPcmFn = samples_to_pcm;

#[cfg(test)]
mod tests {
    use super::*;

    fn samples_from_pcm(pcm_s16le: &[u8]) -> Vec<i16> {
        pcm_s16le
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect()
    }

    fn wav_data_chunk(wav_bytes: &[u8]) -> &[u8] {
        assert_eq!(&wav_bytes[0..4], b"RIFF");
        assert_eq!(&wav_bytes[8..12], b"WAVE");
        assert_eq!(&wav_bytes[12..16], b"fmt ");
        assert_eq!(&wav_bytes[36..40], b"data");

        let data_len =
            u32::from_le_bytes([wav_bytes[40], wav_bytes[41], wav_bytes[42], wav_bytes[43]])
                as usize;
        let data_start = 44;
        let data_end = data_start + data_len;
        assert_eq!(wav_bytes.len(), data_end);
        &wav_bytes[data_start..data_end]
    }

    #[test]
    fn float_audio_to_pcm_clips_and_converts_mono_waveform() {
        let audio = FloatAudioBuffer {
            waveform: vec![-1.5, -0.5, 0.0, 0.5, 1.5],
            sample_rate: 24_000,
            channels: 1,
        };

        let pcm_audio = float_audio_to_pcm(&audio).unwrap();

        assert_eq!(pcm_audio.sample_rate, 24_000);
        assert_eq!(pcm_audio.channels, 1);
        assert_eq!(
            samples_from_pcm(&pcm_audio.pcm_s16le),
            vec![-32767, -16384, 0, 16384, 32767]
        );
    }

    #[test]
    fn float_audio_to_pcm_rejects_non_mono_backend_audio() {
        let audio = FloatAudioBuffer {
            waveform: vec![0.0, 1.0],
            sample_rate: 24_000,
            channels: 2,
        };

        let error = float_audio_to_pcm(&audio).unwrap_err();

        assert!(error.message.contains("mono"));
    }

    #[test]
    fn normalize_audio_downmixes_stereo_to_mono() {
        let audio = AudioBuffer {
            pcm_s16le: b"\x10\x00\x30\x00\x20\x00\x40\x00".to_vec(),
            sample_rate: 24_000,
            channels: 2,
        };

        let normalized = normalize_audio(&audio, 24_000, 1).unwrap();

        assert_eq!(normalized.channels, 1);
        assert_eq!(normalized.pcm_s16le, b"\x20\x00\x30\x00");
    }

    #[test]
    fn normalize_audio_upmixes_mono_to_stereo() {
        let audio = AudioBuffer {
            pcm_s16le: b"\x10\x00\x20\x00".to_vec(),
            sample_rate: 24_000,
            channels: 1,
        };

        let normalized = normalize_audio(&audio, 24_000, 2).unwrap();

        assert_eq!(normalized.channels, 2);
        assert_eq!(normalized.pcm_s16le, b"\x10\x00\x10\x00\x20\x00\x20\x00");
    }

    #[test]
    fn normalize_audio_resamples_when_rate_changes() {
        let audio = AudioBuffer {
            pcm_s16le: samples_to_pcm(&[0, 1000, 2000, 3000]),
            sample_rate: 24_000,
            channels: 1,
        };

        let normalized = normalize_audio(&audio, 16_000, 1).unwrap();

        assert_eq!(normalized.sample_rate, 16_000);
        assert_eq!(normalized.channels, 1);
        assert_eq!(samples_from_pcm(&normalized.pcm_s16le), vec![0, 1500, 3000]);
    }

    #[test]
    fn normalize_audio_rejects_invalid_frame_alignment() {
        let audio = AudioBuffer {
            pcm_s16le: vec![0, 0, 1],
            sample_rate: 24_000,
            channels: 1,
        };

        let error = normalize_audio(&audio, 24_000, 1).unwrap_err();

        assert!(error.message.contains("align"));
    }

    #[test]
    fn serialize_wav_writes_expected_header() {
        let audio = AudioBuffer {
            pcm_s16le: samples_to_pcm(&[0, 1000, -1000, 2500]),
            sample_rate: 24_000,
            channels: 1,
        };

        let wav_bytes = serialize_wav(&audio).unwrap();

        assert_eq!(&wav_bytes[0..4], b"RIFF");
        assert_eq!(&wav_bytes[8..12], b"WAVE");
        assert_eq!(&wav_bytes[12..16], b"fmt ");
        assert_eq!(u16::from_le_bytes([wav_bytes[22], wav_bytes[23]]), 1);
        assert_eq!(
            u32::from_le_bytes([wav_bytes[24], wav_bytes[25], wav_bytes[26], wav_bytes[27]]),
            24_000
        );
        assert_eq!(&wav_bytes[36..40], b"data");
        assert_eq!(
            u32::from_le_bytes([wav_bytes[40], wav_bytes[41], wav_bytes[42], wav_bytes[43]])
                as usize,
            audio.pcm_s16le.len()
        );
        assert_eq!(wav_data_chunk(&wav_bytes), audio.pcm_s16le.as_slice());
    }

    #[test]
    fn serialize_pcm_s16le_returns_raw_pcm_bytes() {
        let audio = AudioBuffer {
            pcm_s16le: b"\x10\x00\x20\x00".to_vec(),
            sample_rate: 24_000,
            channels: 1,
        };

        let pcm = serialize_pcm_s16le(&audio).unwrap();

        assert_eq!(pcm, b"\x10\x00\x20\x00");
        assert_eq!(pcm.len(), 4);
    }
}
