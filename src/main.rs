use rand::{RngCore, SeedableRng};
use rand::rngs::StdRng;
use rayon::prelude::*;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;
use std::time::Duration;

mod helpers;
use helpers::format_number_with_commas;

const DEFAULT_OUTPUT_FILE: &str = "qrandom.bytes";

#[derive(Deserialize)]
struct QRandomResponse {
    #[serde(rename = "binaryURL")]
    binary_url: String,
}

#[derive(Deserialize)]
struct AnuQrngResponse {
    data: Vec<u8>,
    success: bool,
}

fn main() {
    println!("🎲 \x1b[1mQuantum Coin Toss\x1b[0m");
    println!();

    // Parse command line arguments for number of coin flips and output file
    let args: Vec<String> = env::args().collect();
    let mut num_flips = 1;
    let mut output_file = DEFAULT_OUTPUT_FILE.to_string();
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--out" => {
                if i + 1 < args.len() {
                    let requested_file = &args[i + 1];
                    if Path::new(requested_file).exists() {
                        println!("\x1b[33m⚠️  Warning: File '{}' already exists, using default '{}'\x1b[0m", requested_file, DEFAULT_OUTPUT_FILE);
                    } else {
                        output_file = requested_file.clone();
                    }
                    i += 2;
                } else {
                    eprintln!("❌ Missing file path after {}", args[i]);
                    std::process::exit(1);
                }
            }
            _ => {
                match args[i].parse::<usize>() {
                    Ok(n) if n > 0 => num_flips = n,
                    Ok(_) => {
                        eprintln!("❌ Number of flips must be greater than 0");
                        std::process::exit(1);
                    }
                    Err(_) => {
                        eprintln!("❌ Invalid number format: {}", args[i]);
                        std::process::exit(1);
                    }
                }
                i += 1;
            }
        }
    }

    println!("📊 Flips: {}", num_flips);
    println!();

    // Try to fetch bytes from quantum sources
    let (quantum_bytes, is_quantum) = fetch_random_bytes_with_source(1024);
    
    // Save quantum bytes to hex file only if we got them from quantum sources
    if is_quantum {
        save_quantum_bytes_to_file(&quantum_bytes, &output_file);
    }

    let (ones, zeros) = if num_flips == 1 {
        // Single flip: use quantum bytes directly
        println!("🔬 Using quantum entropy directly");
        let (q_ones, q_zeros) = count_bits(&quantum_bytes);
        println!("🎲 Quantum entropy: \x1b[36m{}\x1b[0m 1s : \x1b[36m{}\x1b[0m 0s (ratio: {})", q_ones, q_zeros, format_ratio(q_ones, q_zeros));
        (q_ones, q_zeros)
    } else {
        // Multiple flips: N-1 flips using seeded CSRNG + 1 flip using quantum bytes directly
        if !is_quantum {
            println!("🌱 Using saved quantum entropy to seed {} flips ({} CSRNG + 1 quantum)", num_flips, num_flips - 1);
        } else {
            println!("🌱 Using quantum seed for {} flips ({} CSRNG + 1 quantum)", num_flips, num_flips - 1);
        }
        let (total_ones, total_zeros, q_ones, q_zeros) = perform_multiple_flips(&quantum_bytes, num_flips);
        println!("🎲 Quantum entropy: \x1b[36m{}\x1b[0m 1s : \x1b[36m{}\x1b[0m 0s (ratio: {})", format_number_with_commas(q_ones as u64), format_number_with_commas(q_zeros as u64), format_ratio(q_ones, q_zeros));
        (total_ones, total_zeros)
    };
    
    println!();
    println!("📈 Result: \x1b[36m{}\x1b[0m ones, \x1b[36m{}\x1b[0m zeros", format_number_with_commas(ones as u64), format_number_with_commas(zeros as u64));
    
    if ones > zeros {
        println!("🎯 Outcome: \x1b[1;32mYES\x1b[0m");
    } else {
        println!("🎯 Outcome: \x1b[1;31mNO\x1b[0m");
    }
}

fn count_bits(bytes: &[u8]) -> (u32, u32) {
    let mut ones = 0;
    let mut zeros = 0;
    
    for byte in bytes {
        ones += byte.count_ones();
        zeros += byte.count_zeros();
    }
    
    (ones, zeros)
}

fn format_ratio(ones: u32, zeros: u32) -> String {
    let total = ones + zeros;
    if total == 0 {
        return "0.00".to_string();
    }
    let ratio = ones as f64 / total as f64;
    format!("{:.3}", ratio)
}

fn save_quantum_bytes_to_file(bytes: &[u8], output_file: &str) {
    let hex_string = hex::encode(bytes);
    match fs::write(output_file, hex_string) {
        Ok(_) => println!("💾 Saved quantum entropy to file: \x1b[36m{}\x1b[0m", output_file),
        Err(e) => eprintln!("❌ Failed to save: {}", e),
    }
}

fn load_saved_quantum_bytes() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let hex_string = fs::read_to_string(DEFAULT_OUTPUT_FILE)?;
    let bytes = hex::decode(hex_string.trim())?;
    Ok(bytes)
}

fn perform_multiple_flips(seed_bytes: &[u8], num_flips: usize) -> (u32, u32, u32, u32) {
    // Generate N-1 flips using seeded CSRNG
    let csrng_flips = num_flips - 1;
    let csrng_bytes = csrng_flips * 1024;
    
    if csrng_flips > 0 {
        println!("⚡ Generating \x1b[36m{}\x1b[0m bytes from seeded CSRNG ({} flips)", csrng_bytes, csrng_flips);
    }
    
    // Create seed from quantum bytes (we need exactly 32 bytes for StdRng)
    let mut seed = [0u8; 32];
    if seed_bytes.len() >= 32 {
        seed.copy_from_slice(&seed_bytes[..32]);
    } else {
        // If we have fewer than 32 bytes, repeat the pattern
        for (i, &byte) in seed_bytes.iter().cycle().take(32).enumerate() {
            seed[i] = byte;
        }
    }
    
    // Generate N-1 flips using parallel CSRNG
    let (csrng_ones, csrng_zeros): (u32, u32) = if csrng_flips > 0 {
        (0..csrng_flips)
            .into_par_iter()
            .map(|flip_index| {
                // Create a unique seed for each flip by combining original seed with flip index
                let mut flip_seed = seed;
                let flip_bytes = flip_index.to_le_bytes();
                for (i, &byte) in flip_bytes.iter().enumerate() {
                    if i < flip_seed.len() {
                        flip_seed[i] ^= byte; // XOR with flip index for uniqueness
                    }
                }
                
                // Create RNG for this flip
                let mut rng = StdRng::from_seed(flip_seed);
                let mut bytes = vec![0u8; 1024];
                rng.fill_bytes(&mut bytes);
                
                // Count bits for this flip
                count_bits(&bytes)
            })
            .reduce(|| (0, 0), |acc, (ones, zeros)| (acc.0 + ones, acc.1 + zeros))
    } else {
        (0, 0)
    };
    
    if csrng_flips > 0 {
        println!("✅ Generated \x1b[36m{}\x1b[0m bytes from CSRNG", csrng_bytes);
    }
    
    // Generate the Nth (final) flip using quantum bytes directly
    println!("🔬 Using quantum entropy directly for final flip");
    let (quantum_ones, quantum_zeros) = count_bits(seed_bytes);
    
    // Combine results
    let total_ones = csrng_ones + quantum_ones;
    let total_zeros = csrng_zeros + quantum_zeros;
    
    (total_ones, total_zeros, quantum_ones, quantum_zeros)
}

fn fetch_random_bytes_with_source(num_bytes: usize) -> (Vec<u8>, bool) {
    // Create a client with timeout settings
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client");
    
    // Try ANU QRNG first (cap at 1024 bytes due to API limitations)
    let anu_bytes_to_fetch = std::cmp::min(num_bytes, 1024);
    
    println!("🔍 \x1b[33mTrying ANU QRNG...\x1b[0m");
    match fetch_anu_qrng_bytes(&client, anu_bytes_to_fetch) {
        Ok(bytes) => {
            println!("✅ ANU QRNG: Received \x1b[32m{} bytes\x1b[0m", bytes.len());
            return (bytes, true); // True indicates quantum source
        }
        Err(e) => {
            eprintln!("❌ ANU QRNG: \x1b[31m{}\x1b[0m", e);
            println!("🔄 \x1b[33mTrying qrandom.io...\x1b[0m");
        }
    }
    
    // Fallback to qrandom.io
    match fetch_qrandom_bytes(&client, num_bytes) {
        Ok(bytes) => {
            println!("✅ qrandom.io: Received \x1b[32m{} bytes\x1b[0m", bytes.len());
            return (bytes, true); // True indicates quantum source
        }
        Err(e) => {
            eprintln!("❌ qrandom.io: \x1b[31m{}\x1b[0m", e);
            println!("🔄 \x1b[33mFalling back to CSRNG...\x1b[0m");
        }
    }
    
    // Final fallback to cryptographic SRNG (not quantum)
    match fetch_crypto_srng_bytes(num_bytes) {
        Ok(bytes) => {
            println!("✅ CSRNG: \x1b[32m{} bytes\x1b[0m", bytes.len());
            return (bytes, false); // False indicates non-quantum source
        }
        Err(e) => {
            eprintln!("❌ CSRNG: \x1b[31m{}\x1b[0m", e);
        }
    }

    // Last resort: try to reuse saved quantum bytes
    match load_saved_quantum_bytes() {
        Ok(bytes) => {
            println!("♻️  Reusing saved quantum entropy from file: \x1b[36m{}\x1b[0m", DEFAULT_OUTPUT_FILE);
            return (bytes, true); // True since these are quantum bytes
        }
        Err(e) => {
            eprintln!("❌ No saved entropy: \x1b[31m{}\x1b[0m", e);
        }
    }

    eprintln!("💥 All entropy sources failed");
    std::process::exit(1);
}

fn fetch_qrandom_bytes(client: &Client, num_bytes: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let url = format!("https://qrandom.io/api/random/binary?bytes={}", num_bytes);
    
    let response = client.get(url).send()?;
    
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()).into());
    }
    
    let json_response: QRandomResponse = response.json()?;
    
    let binary_response = client.get(&json_response.binary_url).send()?;
    
    if !binary_response.status().is_success() {
        return Err(format!("Binary fetch HTTP {}", binary_response.status()).into());
    }
    
    let bytes = binary_response.bytes()?.to_vec();
    
    Ok(bytes)
}

fn fetch_anu_qrng_bytes(client: &Client, num_bytes: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut all_bytes = Vec::new();
    let mut remaining = num_bytes;
    
    // ANU QRNG has a maximum of 1024 elements per request
    while remaining > 0 {
        let chunk_size = std::cmp::min(remaining, 1024);
        let url = format!("https://qrng.anu.edu.au/API/jsonI.php?length={}&type=uint8", chunk_size);
        
        let response = client.get(&url).send()?;
        
        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()).into());
        }
        
        let anu_response: AnuQrngResponse = response.json()?;
        
        if !anu_response.success {
            return Err("API returned success=false".into());
        }
        
        if anu_response.data.len() != chunk_size {
            return Err(format!("Expected {} bytes, got {}", chunk_size, anu_response.data.len()).into());
        }
        
        all_bytes.extend(anu_response.data);
        remaining -= chunk_size;
        
        // Small delay between requests to be respectful to the API
        if remaining > 0 {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    
    Ok(all_bytes)
}

fn fetch_crypto_srng_bytes(num_bytes: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut rng = rand::rng();
    let mut bytes = vec![0u8; num_bytes];
    
    // Fill the vector with cryptographically secure random bytes
    rng.fill_bytes(&mut bytes);
    
    Ok(bytes)
}