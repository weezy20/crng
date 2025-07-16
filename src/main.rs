use rand::rngs::StdRng;
use rand::SeedableRng;
use rand::Rng;
use rayon::prelude::*;
use reqwest::blocking::get;
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

    // Use a fixed maximum number of RNGs (32) for optimal parallelization
    // More RNGs doesn't improve randomness, just increases overhead
    let max_rngs = 32;
    let bytes_per_rng = 32;
    let available_full_seeds = num_bytes / bytes_per_rng;
    let num_rngs = std::cmp::min(max_rngs, std::cmp::max(1, available_full_seeds));
    
    println!("Using {} RNGs (max {}) with {}-byte seeds", 
             num_rngs, max_rngs, bytes_per_rng);

    let seeds: Vec<[u8; 32]> = if num_bytes >= bytes_per_rng {
        // Create seeds from available bytes, cycling through if we have excess
        (0..num_rngs)
            .map(|i| {
                let mut seed = [0u8; 32];
                for j in 0..32 {
                    let byte_index = (i * 32 + j) % num_bytes;
                    seed[j] = qrandom_bytes[byte_index];
                }
                seed
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
    let total_target_bits = 40_000_000u64;
    let base_items_per_rng = (total_target_bits / 8) / num_rngs as u64;
    let remainder_bytes = ((total_target_bits / 8) % num_rngs as u64) as usize;
    
    println!("Each of {} RNGs will generate {} bytes (+ {} RNGs get 1 extra byte)", 
             num_rngs, base_items_per_rng, remainder_bytes);
    println!("This ensures exactly {} total bits", total_target_bits);

    // Generate random bytes and count set bits in parallel
    let partial_sums: Vec<u64> = seeds.par_iter().enumerate()
        .map(|(i, &seed)| {
            let mut rng = StdRng::from_seed(seed);
            let items_for_this_rng = if i < remainder_bytes { 
                base_items_per_rng + 1 
            } else { 
                base_items_per_rng 
            };
            (0..items_for_this_rng)
                .map(|_| rng.random::<u8>())
                .map(|byte| byte.count_ones() as u64)
                .sum::<u64>()
        })
        .collect();

    // Sum trues and compute falses
    let total_trues: u64 = partial_sums.iter().sum();
    let total_bits: u64 = total_target_bits;
    let total_falses = total_bits - total_trues;
    let diff = total_trues as i64 - total_falses as i64;

    println!("Generated {} total bits: {} ones, {} zeros", 
             total_bits, total_trues, total_falses);
    println!("Ratio: {:.6} ones per bit", total_trues as f64 / total_bits as f64);

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