#![allow(clippy::missing_safety_doc)]

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
compile_error!("Unsupported OS");

// no_std support is not available until symphonia does (#88)

use std::io::{self, Cursor};
use std::ptr;
use std::slice;

use symphonia::core::{
    audio::SampleBuffer,
    codecs::Decoder,
    errors::Error as SymphoniaError,
    formats::{FormatReader, Track},
    io::MediaSourceStream,
};
use symphonia::default::{codecs::FlacDecoder, formats::FlacReader};

#[repr(C)]
pub struct ProbeResult {
    success: bool,
    length_samples: i32,
    channels: i32,
    frequency: i32,
    reader: *mut FlacReader,
}

impl Default for ProbeResult {
    fn default() -> Self {
        Self {
            success: false,
            length_samples: 0,
            channels: 0,
            frequency: 0,
            reader: ptr::null_mut(),
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn probe(data: *const u8, data_size: i32) -> ProbeResult {
    let slice = slice::from_raw_parts(data, data_size as usize);
    let source = Box::new(Cursor::new(slice));
    let mss = MediaSourceStream::new(source, Default::default());

    let mut res = ProbeResult::default();

    let Ok(reader) = FlacReader::try_new(mss, &Default::default()) else {
		return res;
	};

    let Track { codec_params, .. } = reader.default_track().unwrap();

    res.length_samples = codec_params.n_frames.unwrap() as i32;
    res.channels = codec_params.channels.unwrap().count() as i32;
    res.frequency = codec_params.sample_rate.unwrap() as i32;

    res.reader = Box::into_raw(Box::new(reader));
    res.success = true;

    res
}

#[no_mangle]
pub unsafe extern "C" fn decode(
    reader: *mut FlacReader,
    buffer: *mut f32,
    buffer_size: i32,
) -> bool {
    let mut reader = Box::from_raw(reader);

    let slice = slice::from_raw_parts_mut(buffer, buffer_size as usize);
    let mut pos = 0usize;

    let Track {
        codec_params,
        id: track_id,
        ..
    } = reader.default_track().unwrap();
    let track_id = *track_id;

    let Ok(mut decoder) = FlacDecoder::try_new(codec_params, &Default::default()) else {
		return false;
	};

    loop {
        let packet = match reader.next_packet() {
            Ok(packet) => packet,
            Err(err) => {
                break matches!(err, SymphoniaError::IoError(e) if e.kind() == io::ErrorKind::UnexpectedEof)
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        let Ok(decoded) = decoder.decode(&packet) else {
			break false;
		};

        let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
        buf.copy_interleaved_ref(decoded);
		let buf = buf;

        let samples = buf.samples();
        let len = samples.len();

        slice[pos..pos + len].copy_from_slice(samples);
        pos += len;
    }
}
