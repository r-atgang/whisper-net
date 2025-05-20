use std::collections::HashMap;      // hashmaps for the data
use std::sync::{Arc, Mutex};     // arc to share pointer ownership and mutex for thread locking
use std::net::UdpSocket;        // UDP protocol shih
use std::thread;            // threading
use std::time::Duration;
use std::collections::VecDeque;         // we love double-ended vectors
use std::io::Write;                                        

type AudioBuffer = Arc<Mutex<HashMap<String, VecDeque<i16>>>>;      // type for abuffer

const PORT: u16 = 6969;        // port number running on 60060
fn main() -> std::io::Result<()> {
    let socket = Arc::new(UdpSocket::bind(format!("0.0.0.0:{}", PORT))?);
    println!("Listening on UDP port {}", PORT);
    let buffers: AudioBuffer = Arc::new(Mutex::new(HashMap::new()));
    
    //  Thread 1: receiver for data transmission
    {
            let socket = Arc::clone(&socket);       // sharing ownership of socket
            let buffers = Arc::clone(&buffers);     // sharing ownership of buffers
            
            // START THREAD
            thread::spawn(move || {
                let mut buffer = [0u8; 1024]; // array of 1024 u8 elements

                loop {
                    match socket.recv_from(&mut buffer) {
                        Ok((amt, src)) => {
                            let key = src.to_string();

                            // AUDIO CHUNKS 
                            let audio_chunk: Vec<i16> = buffer[..amt]
                                .chunks_exact(2)        // chunk size is 2 bytes
                                .map(|b| i16::from_le_bytes([b[0], b[1]]))
                                .collect();            // little-endian bytes to i16

                            let mut map = buffers.lock().unwrap();
                            let entry = map.entry(key).or_insert_with(VecDeque::new);
                            entry.extend(audio_chunk);
                            if entry.len() > 16000 {
                                entry.drain(..(entry.len() - 16000)); // limit buffer
                            }
                        }
                        Err(e) => {
                            eprintln!("Error receiving from socket: {}", e);
                        }
                    }
                }
            });    
    }       // END THREAD

    // Thread 2: mixer & playback
    // using ALSA
    {
        let buffers = Arc::clone(&buffers);     // sharing ownership of buffers

        // START THREAD
        thread::spawn(move || {
            use alsa::pcm::{PCM, HwParams, Format, Access};
            
            // Pulse Code Modulation interfaces the ALSA device; sending audio; non-blocking
            let pcm = PCM::new("default", alsa::Direction::Playback, false).unwrap();

            // Hardware parameter settings
            let hwp = HwParams::any(&pcm).unwrap();
            hwp.set_channels(1).unwrap();       // set playback to mono
            hwp.set_rate(16000, alsa::ValueOr::Nearest).unwrap();       // sample rate of 16k samples/sec   
            hwp.set_format(Format::s16()).unwrap();         // audio format is signed 16bit
            hwp.set_access(Access::RWInterleaved).unwrap();     // access mode: r/w interleaved
            pcm.hw_params(&hwp).unwrap();        // hw params set on pcm pointing to hwp

            let io = pcm.io_i16().unwrap();

            let mut output_file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open("mixed_output.raw")
                .unwrap();

            loop {
                thread::sleep(Duration::from_millis(20));       // 20ms frame
                
                let mut mixed = vec![0i32; 320];        // intermediate buffer
                let mut active_sources = 0;     // to hold the amount of active sources

                {
                    let mut map = buffers.lock().unwrap();      // safe access to data
                    for (_ip, buffer) in map.iter_mut() {
                        if buffer.len() >= 320 {
                            // if buffer is 320 elements, drain from buffer into vec
                            let samples: Vec<_> = buffer.drain(..320).collect();
                            for (i, sample) in samples.iter().enumerate() {
                                mixed[i] += *sample as i32;
                            }
                            active_sources += 1;        // tally active source
                        }
                    }
                }
                
                // make output if active sources
                let output: Vec<i16> = if active_sources > 0 {
                    // audio mixing pattern; iterate over "mixed"
                    mixed.iter().map(|v| 
                                     (*v / active_sources as i32)       // % by sources
                                     // clamp to i16 range for safety
                                     .clamp(i16::MIN as i32, i16::MAX as i32)
                                     // convert to i16
                                     as i16
                                )
                                .collect()
                } else {
                    vec![0i16; 320]
                };
                
                // Play the output!
                io.writei(&output).unwrap();
                let raw_bytes = output.iter().flat_map(|s| s.to_le_bytes()).collect::<Vec<u8>>();
                output_file.write_all(&raw_bytes).unwrap();
            }
        });
    }

    // main thread loop
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

