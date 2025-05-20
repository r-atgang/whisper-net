use std::collections::{HashMap, VecDeque};
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

type AudioBuffer = Arc<Mutex<HashMap<String, VecDeque<i16>>>>;

const PORT: u16 = 6969;
const SAMPLE_RATE: u32 = 16000;
const CHANNELS: u16 = 1;
const FRAME_SIZE: usize = 320; // 20ms @ 16kHz
const MAX_BUFFER_LEN: usize = SAMPLE_RATE as usize; // 1s per stream
const JITTER_FRAMES: usize = 3;

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", PORT))?;
    println!("Listening for TCP connections on port {}", PORT);
    let buffers: AudioBuffer = Arc::new(Mutex::new(HashMap::new()));

    // === Thread 1: TCP client handler ===
    {
        let buffers = Arc::clone(&buffers);
        thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let peer = stream
                            .peer_addr()
                            .map(|addr| addr.to_string())
                            .unwrap_or_else(|_| "unknown".to_string());

                        println!("New TCP connection from {}", peer);
                        let buffers = Arc::clone(&buffers);
                        thread::spawn(move || handle_client(stream, peer, buffers));
                    }
                    Err(e) => eprintln!("Failed to accept connection: {}", e),
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

            let mut output_file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open("mixed_output.wav")
                .unwrap();
            write_wav_header(&mut output_file, 0).unwrap();

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

                let raw_bytes: Vec<u8> = output.iter().flat_map(|s| s.to_le_bytes()).collect();
                output_file.write_all(&raw_bytes).unwrap();
            }
        });
    }

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

fn handle_client(mut stream: TcpStream, peer: String, buffers: AudioBuffer) {
    let mut buffer = [0u8; 1024];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                println!("Client {} disconnected", peer);
                break;
            }
            Ok(n) => {
                let samples: Vec<i16> = buffer[..n]
                    .chunks_exact(2)
                    .map(|b| i16::from_le_bytes([b[0], b[1]]))
                    .collect();

                let chunk_len = samples.len();

                let mut map = buffers.lock().unwrap();
                let entry = map.entry(peer.clone()).or_insert_with(VecDeque::new);
                entry.extend(samples);

                if entry.len() > MAX_BUFFER_LEN {
                    println!(
                        "Buffer too large for {} ({} samples), trimming...",
                        peer,
                        entry.len()
                    );
                    entry.drain(..(entry.len() - MAX_BUFFER_LEN));
                }

                println!(
                    "Received {} bytes ({} samples) from {} (buffer len = {})",
                    n,
                    chunk_len,
                    peer,
                    entry.len()
                );
            }
            Err(e) => {
                eprintln!("Read error from {}: {}", peer, e);
                break;
            }
        }
    }

    // Cleanup
    let mut map = buffers.lock().unwrap();
    map.remove(&peer);
    println!("Removed buffer for {}", peer);
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
