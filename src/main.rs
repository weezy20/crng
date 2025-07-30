use rand::{RngCore, SeedableRng};
use rand::rngs::StdRng;
use rayon::prelude::*;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::time::Duration;
use clap::Parser;

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

/// Quantum Coin Toss - Generate truly random coin flips using quantum entropy
#[derive(Parser)]
#[command(name = "qcoin")]
#[command(about = "A quantum random number generator for coin tosses")]
#[command(long_about = "Generate truly random coin flips using quantum entropy sources like ANU QRNG and qrandom.io. Fallback to cryptographically secure RNG when quantum sources are unavailable.")]
#[command(version)]
struct Args {
    /// Number of coin flips to perform
    #[arg(short = 'n', long = "number", value_name = "NUM_FLIPS", default_value = "1")]
    num_flips: usize,
    
    /// Output file for quantum entropy bytes (hex format)
    #[arg(short = 'o', long = "output", value_name = "FILE", default_value = DEFAULT_OUTPUT_FILE)]
    output_file: String,
    
    /// Source file to use as entropy source instead of quantum sources.
    /// File can contain hex string (e.g., "abc123", "0xabc123") or raw binary data.
    /// Hex strings are automatically detected and decoded.
    #[arg(short = 's', long = "source", value_name = "FILE", conflicts_with = "output_file")]
    source_file: Option<String>,
    
    /// Hex string to use as entropy source instead of quantum sources.
    /// Can include optional 0x prefix (e.g., "abc123", "0xabc123").
    #[arg(long = "hex", value_name = "HEX_STRING", conflicts_with_all = ["source_file"])]
    hex_string: Option<String>,
}

fn main() {
    // Parse command line arguments using clap
    let args = Args::parse();
    
    println!("🎲 \x1b[1mQuantum Coin Toss\x1b[0m");
    println!();

    // Validate number of flips
    if args.num_flips == 0 {
        eprintln!("❌ Number of flips must be greater than 0");
        std::process::exit(1);
    }

    // Check if output file already exists and warn user
    if Path::new(&args.output_file).exists() && args.output_file != DEFAULT_OUTPUT_FILE {
        println!("\x1b[33m⚠️  Warning: File '{}' already exists, it may be overwritten\x1b[0m", args.output_file);
    }

    println!("📊 Flips: {}", args.num_flips);
    println!();

    // Determine entropy source and fetch bytes
    let (entropy_bytes, is_quantum, source_description) = if let Some(hex_string) = &args.hex_string {
        // Use hex string as entropy
        match parse_hex_string(hex_string) {
            Ok(bytes) => {
                if bytes.is_empty() {
                    eprintln!("❌ Hex string is empty");
                    std::process::exit(1);
                }
                
                let description = if args.num_flips == 1 {
                    // For single flip, always use bytes directly
                    format!("🔤 Using hex string entropy ({} bytes - direct interpretation)", bytes.len())
                } else if bytes.len() == 1024 {
                    "🔤 Using hex string entropy (1024 bytes - perfect match)".to_string()
                } else if bytes.len() < 1024 {
                    format!("🔤 Using hex string entropy ({} bytes < 1024 - will seed CSRNG)", bytes.len())
                } else {
                    format!("🔤 Using hex string entropy ({} bytes > 1024 - will seed CSRNG)", bytes.len())
                };
                
                (bytes, false, description)
            },
            Err(e) => {
                eprintln!("❌ Failed to parse hex string: {}", e);
                std::process::exit(1);
            }
        }
    } else if let Some(source_file) = &args.source_file {
        // Use source file as entropy
        match read_source_file(source_file) {
            Ok(bytes) => {
                if bytes.is_empty() {
                    eprintln!("❌ Source file is empty");
                    std::process::exit(1);
                }
                
                let description = if args.num_flips == 1 {
                    // For single flip, always use bytes directly
                    format!("📁 Using file entropy ({} bytes - direct interpretation)", bytes.len())
                } else if bytes.len() == 1024 {
                    "📁 Using file entropy (1024 bytes - perfect match)".to_string()
                } else if bytes.len() < 1024 {
                    format!("📁 Using file entropy ({} bytes < 1024 - will seed CSRNG)", bytes.len())
                } else {
                    format!("📁 Using file entropy ({} bytes > 1024 - will seed CSRNG)", bytes.len())
                };
                
                (bytes, false, description)
            },
            Err(e) => {
                eprintln!("❌ Failed to read source file '{}': {}", source_file, e);
                std::process::exit(1);
            }
        }
    } else {
        // Use quantum sources as before
        let (quantum_bytes, is_quantum) = fetch_random_bytes_with_source(1024);
        let description = if is_quantum {
            "🌱 Using quantum entropy sources".to_string()
        } else {
            "🌱 Using saved quantum entropy".to_string()
        };
        (quantum_bytes, is_quantum, description)
    };
    
    println!("{}", source_description);
    
    // Save quantum bytes to hex file only if we got them from quantum sources and not using source file
    if is_quantum && args.source_file.is_none() {
        save_quantum_bytes_to_file(&entropy_bytes, &args.output_file);
    } else if args.hex_string.is_some() {
        // Save hex string entropy to file for reuse
        save_quantum_bytes_to_file(&entropy_bytes, &args.output_file);
        println!("💾 Hex string entropy saved for future reuse");
    }

    let (ones, zeros) = if args.num_flips == 1 {
        // Single flip: use entropy bytes directly
        println!("🔬 Using entropy directly");
        let (q_ones, q_zeros) = count_bits(&entropy_bytes);
        println!("🎲 Entropy bits: \x1b[36m{}\x1b[0m 1s : \x1b[36m{}\x1b[0m 0s (ratio: {})", q_ones, q_zeros, format_ratio(q_ones, q_zeros));
        (q_ones, q_zeros)
    } else {
        // Multiple flips: N-1 flips using seeded CSRNG + 1 flip using entropy bytes directly
        if entropy_bytes.len() < 1024 {
            println!("🌱 Using {} bytes to seed {} flips ({} CSRNG + 1 direct)", entropy_bytes.len(), args.num_flips, args.num_flips - 1);
        } else {
            println!("🌱 Using entropy to seed {} flips ({} CSRNG + 1 direct)", args.num_flips, args.num_flips - 1);
        }
        let (total_ones, total_zeros, q_ones, q_zeros) = perform_multiple_flips(&entropy_bytes, args.num_flips);
        println!("🎲 Direct entropy: \x1b[36m{}\x1b[0m 1s : \x1b[36m{}\x1b[0m 0s (ratio: {})", format_number_with_commas(q_ones as u64), format_number_with_commas(q_zeros as u64), format_ratio(q_ones, q_zeros));
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

fn read_source_file(file_path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // First try to read as text (for hex strings)
    match fs::read_to_string(file_path) {
        Ok(content) => {
            let trimmed = content.trim();
            
            if trimmed.len() > 0 {
                // Try to handle hex string (with or without 0x prefix)
                let hex_str = if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
                    &trimmed[2..] // Remove 0x prefix
                } else {
                    trimmed
                };
                
                // Check if it looks like a hex string (only contains hex characters and even length)
                if hex_str.len() > 0 && hex_str.len() % 2 == 0 && hex_str.chars().all(|c| c.is_ascii_hexdigit()) {
                    // Try to decode as hex
                    match hex::decode(hex_str) {
                        Ok(bytes) => {
                            println!("📁 Reading {} bytes from hex string in source file: \x1b[36m{}\x1b[0m", bytes.len(), file_path);
                            return Ok(bytes);
                        },
                        Err(_) => {
                            // Fall through to binary read
                        }
                    }
                }
            }
            
            // If not a valid hex string, treat the text content as raw bytes
            let bytes = content.as_bytes().to_vec();
            println!("📁 Reading {} bytes from text file as raw bytes: \x1b[36m{}\x1b[0m", bytes.len(), file_path);
            Ok(bytes)
        },
        Err(_) => {
            // If reading as text fails, read as binary
            let bytes = fs::read(file_path)?;
            println!("📁 Reading {} bytes from binary file: \x1b[36m{}\x1b[0m", bytes.len(), file_path);
            Ok(bytes)
        }
    }
}

fn parse_hex_string(hex_input: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let trimmed = hex_input.trim();
    
    // Handle hex string (with or without 0x prefix)
    let hex_str = if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
        &trimmed[2..] // Remove 0x prefix
    } else {
        trimmed
    };
    
    // Validate hex string
    if hex_str.is_empty() {
        return Err("Empty hex string".into());
    }
    
    if hex_str.len() % 2 != 0 {
        return Err("Hex string must have even length".into());
    }
    
    if !hex_str.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Hex string contains invalid characters".into());
    }
    
    // Decode hex string
    let bytes = hex::decode(hex_str)?;
    println!("🔤 Parsing {} bytes from hex string: \x1b[36m{}\x1b[0m", bytes.len(), hex_str);
    Ok(bytes)
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