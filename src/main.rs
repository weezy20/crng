use rand::rngs::StdRng;
use rand::SeedableRng;
use rand::Rng;
use rayon::prelude::*;
use reqwest::blocking::get;
use std::convert::TryInto;
use serde::Deserialize;
use std::fs;
use std::env;

#[derive(Deserialize)]
struct QRandomResponse {
    #[serde(rename = "binaryURL")]
    binary_url: String,
}

fn main() {
    // Parse command line arguments for number of bytes
    let args: Vec<String> = env::args().collect();
    let num_bytes = if args.len() > 1 {
        args[1].parse::<usize>().unwrap_or(1024)
    } else {
        1024
    };
    
    println!("Fetching {} bytes from qrandom.io", num_bytes);
    
    // Fetch bytes from qrandom.io
    let qrandom_bytes = fetch_qrandom_bytes(num_bytes);
    
    // Create hex string representation
    let hex_string = hex::encode(&qrandom_bytes);
    // Save hex string to file in current working directory
    fs::write("qrandom_bytes.hex", &hex_string).expect("Failed to write hex file");
    println!("Saved {} hex bytes to qrandom_bytes.hex", qrandom_bytes.len());
    
    assert_eq!(qrandom_bytes.len(), num_bytes, "Expected {} bytes from qrandom.io", num_bytes);

    // Determine optimal number of RNGs based on available bytes
    // Use at least 1 RNG, and create as many 32-byte seeds as possible
    let bytes_per_rng = 32;
    let num_rngs = std::cmp::max(1, num_bytes / bytes_per_rng);
    let bytes_used_for_seeds = num_rngs * bytes_per_rng;
    
    println!("Using {} RNGs with {}-byte seeds ({} bytes total for seeds)", 
             num_rngs, bytes_per_rng, bytes_used_for_seeds);

    let seeds: Vec<[u8; 32]> = if num_bytes >= bytes_per_rng {
        // We have enough bytes for at least one full seed
        (0..num_rngs)
            .map(|i| {
                let start = i * bytes_per_rng;
                let end = start + bytes_per_rng;
                qrandom_bytes[start..end].try_into().expect("Slice conversion failed")
            })
            .collect()
    } else {
        // Pad the available bytes to create one 32-byte seed
        let mut seed = [0u8; 32];
        for (i, &byte) in qrandom_bytes.iter().enumerate() {
            seed[i % 32] ^= byte; // XOR to mix the entropy
        }
        vec![seed]
    };

    // Each RNG generates a fixed number of bytes to maintain consistent total output
    let total_target_bits = 5_000_000 * 8; // Same as original: 40 million bits
    let items_per_rng = total_target_bits / (num_rngs * 8); // bits per RNG / 8 bits per byte
    
    println!("Each of {} RNGs will generate {} bytes ({} total bits)", 
             num_rngs, items_per_rng, total_target_bits);

    // Generate random bytes and count set bits in parallel
    let partial_sums: Vec<u64> = seeds.par_iter()
        .map(|&seed| {
            let mut rng = StdRng::from_seed(seed);
            (0..items_per_rng)
                .map(|_| rng.random::<u8>())
                .map(|byte| byte.count_ones() as u64)
                .sum::<u64>()
        })
        .collect();

    // Sum trues and compute falses
    let total_trues: u64 = partial_sums.iter().sum();
    let total_bits: u64 = total_target_bits as u64;
    let total_falses = total_bits - total_trues;
    let diff = total_trues as i64 - total_falses as i64;

    println!("Generated {} total bits: {} ones, {} zeros", 
             total_bits, total_trues, total_falses);

    // Print result based on difference
    if diff > 0 {
        println!("\x1b[1;32myes by {} votes\x1b[0m", diff); // Bold green
    } else if diff < 0 {
        println!("\x1b[1;31mno by {} votes\x1b[0m", -diff); // Bold red
    } else {
        println!("MIRACLE! It's a tie");
    }
}

fn fetch_qrandom_bytes(num_bytes: usize) -> Vec<u8> {
    let url = format!("https://qrandom.io/api/random/binary?bytes={}", num_bytes);
    let response = get(&url).expect("Failed to fetch from qrandom.io");
    let json_response: QRandomResponse = response.json().expect("Failed to parse JSON response");
    
    // Now fetch the actual binary data from the binaryURL
    let binary_response = get(&json_response.binary_url).expect("Failed to fetch binary data");
    let bytes = binary_response.bytes().expect("Failed to get bytes").to_vec();
    
    assert_eq!(bytes.len(), num_bytes, "Expected {} bytes from binary URL", num_bytes);
    bytes
}