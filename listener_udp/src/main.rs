use std::collections::{HashMap, VecDeque};  // hashmaps for the data + we love double-ended vectors
use std::io::{Seek, SeekFrom, Write};       // seek/write for WAV header
use std::net::UdpSocket;                    // UDP protocol shih
use std::sync::{Arc, Mutex};                // arc to share pointer ownership and mutex for thread locking
use std::thread;                            // threading
use std::time::Duration;                    // timing

type AudioBuffer = Arc<Mutex<HashMap<String, VecDeque<i16>>>>; // type for abuffer

const PORT: u16 = 6969;        // port number running on 6969
const SAMPLE_RATE: u32 = 16000;
const CHANNELS: u16 = 1;
const FRAME_SIZE: usize = 320; // 20ms @ 16kHz
const MAX_BUFFER_LEN: usize = SAMPLE_RATE as usize; // 1s buffer per stream
const JITTER_FRAMES: usize = 3; // Jitter buffer (3 * 20ms = 60ms)

fn main() -> std::io::Result<()> {
    let socket = Arc::new(UdpSocket::bind(format!("0.0.0.0:{}", PORT))?);
    socket.set_nonblocking(true)?;
    println!("Listening on UDP port {}", PORT);
    let buffers: AudioBuffer = Arc::new(Mutex::new(HashMap::new()));

    // === Thread 1: UDP listener ===
    {
        let socket = Arc::clone(&socket);
        let buffers = Arc::clone(&buffers);
        thread::spawn(move || {
            let mut buffer = [0u8; 1024];

            loop {
                match socket.recv_from(&mut buffer) {
                    Ok((amt, src)) => {
                        let key = src.to_string(); // use source IP as key
                        let audio_chunk: Vec<i16> = buffer[..amt]
                            .chunks_exact(2)
                            .map(|b| i16::from_le_bytes([b[0], b[1]]))
                            .collect();

                        let chunk_len = audio_chunk.len(); // store length before move

                        let mut map = buffers.lock().unwrap();
                        let entry = map.entry(key.clone()).or_insert_with(|| {
                            println!("New audio stream from {}", key);
                            VecDeque::new()
                        });
                        entry.extend(audio_chunk);

                        println!(
                            "Received {} bytes ({} samples) from {} (buffer len = {})",
                            amt,
                            chunk_len,
                            key,
                            entry.len()
                        );

                        if entry.len() > MAX_BUFFER_LEN {
                            println!(
                                "Buffer too large for {} ({} samples), trimming...",
                                key,
                                entry.len()
                            );
                            entry.drain(..(entry.len() - MAX_BUFFER_LEN));
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(e) => eprintln!("Error receiving from socket: {}", e),
                }
            }
        });
    }

    // === Thread 2: Mixer & playback ===
    {
        let buffers = Arc::clone(&buffers);
        thread::spawn(move || {
            use alsa::pcm::{Access, Format, HwParams, PCM};
            let pcm = PCM::new("default", alsa::Direction::Playback, false)
                .expect("Failed to open ALSA device");
            let hwp = HwParams::any(&pcm).expect("Failed to get ALSA hw params");
            hwp.set_channels(CHANNELS as u32).unwrap();
            hwp.set_rate(SAMPLE_RATE, alsa::ValueOr::Nearest).unwrap();
            hwp.set_format(Format::s16()).unwrap();
            hwp.set_access(Access::RWInterleaved).unwrap();
            pcm.hw_params(&hwp).unwrap();
            let io = pcm.io_i16().unwrap();

            println!("ALSA playback initialized ({} Hz, mono)", SAMPLE_RATE);

            // WAV output file setup
            let mut output_file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open("mixed_output.wav")
                .unwrap();
            write_wav_header(&mut output_file, 0).unwrap(); // temp header

            println!("Output file created: mixed_output.wav");

            loop {
                thread::sleep(Duration::from_millis(20));
                let mut mixed = vec![0i32; FRAME_SIZE];
                let mut active_sources = 0;

                {
                    let mut map = buffers.lock().unwrap();
                    for (ip, buffer) in map.iter_mut() {
                        if buffer.len() >= FRAME_SIZE * JITTER_FRAMES {
                            let samples: Vec<_> = buffer.drain(..FRAME_SIZE).collect();
                            for (i, sample) in samples.iter().enumerate() {
                                mixed[i] += *sample as i32;
                            }
                            active_sources += 1;
                            println!("Mixed frame from {}", ip);
                        }
                    }
                }

                let output: Vec<i16> = if active_sources > 0 {
                    mixed.iter()
                        .map(|v| (*v / active_sources as i32)
                            .clamp(i16::MIN as i32, i16::MAX as i32) as i16)
                        .collect()
                } else {
                    println!("No active sources - playing silence");
                    vec![0i16; FRAME_SIZE]
                };

                match io.writei(&output) {
                    Ok(_) => println!("Played {} samples", output.len()),
                    Err(e) => eprintln!("ALSA write error: {}", e),
                }

                let raw_bytes: Vec<u8> = output.iter()
                    .flat_map(|s| s.to_le_bytes())
                    .collect();
                output_file.write_all(&raw_bytes).unwrap();
            }
        });
    }

    // === Main thread: keep running ===
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

// === WAV header writing ===
fn write_wav_header<W: Write>(writer: &mut W, data_len: u32) -> std::io::Result<()> {
    let header_len = 44;
    let byte_rate = SAMPLE_RATE * CHANNELS as u32 * 2;
    let block_align = CHANNELS * 2;

    writer.write_all(b"RIFF")?;
    writer.write_all(&(data_len + header_len as u32 - 8).to_le_bytes())?;
    writer.write_all(b"WAVEfmt ")?;
    writer.write_all(&(16u32).to_le_bytes())?;
    writer.write_all(&(1u16).to_le_bytes())?;
    writer.write_all(&CHANNELS.to_le_bytes())?;
    writer.write_all(&SAMPLE_RATE.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&(16u16).to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_len.to_le_bytes())?;
    Ok(())
}
