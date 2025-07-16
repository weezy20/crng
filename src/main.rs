use rand::rngs::StdRng;
use rand::SeedableRng;
use rand::Rng;
use rayon::prelude::*;
use reqwest::blocking::get;
use serde::Deserialize;
use std::fs;
use std::env;
use sha2::{Sha256, Digest};

const MAX_RNGS: usize = 32;
const BYTES_PER_RNG: usize = 32;
const TOTAL_TARGET_BITS: u64 = 40_000_000;

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

    let available_full_seeds = num_bytes / BYTES_PER_RNG;
    let num_rngs = std::cmp::min(MAX_RNGS, std::cmp::max(1, available_full_seeds));
    
    println!("Using {} RNGs (max {}) with {}-byte seeds", 
             num_rngs, MAX_RNGS, BYTES_PER_RNG);

    // Better seed generation using SHA-256
    let seeds: Vec<[u8; 32]> = if num_bytes >= BYTES_PER_RNG {
        (0..num_rngs).into_par_iter()
            .map(|i| {
                let start = i * num_bytes / num_rngs;
                let end = (i + 1) * num_bytes / num_rngs;
                let slice = &qrandom_bytes[start..end];
                let mut hasher = Sha256::new();
                hasher.update(slice);
                hasher.update(&i.to_be_bytes()); // Add index as extra entropy
                hasher.finalize().into()
            })
            .collect()
    } else {
        let mut hasher = Sha256::new();
        hasher.update(&qrandom_bytes);
        vec![hasher.finalize().into()]
    };

    let total_target_bits = TOTAL_TARGET_BITS;
    let base_items_per_rng = (total_target_bits / 8) / num_rngs as u64;
    let remainder_bytes = ((total_target_bits / 8) % num_rngs as u64) as usize;
    
    println!("Each of {} RNGs will generate {} bytes (+ {} RNGs get 1 extra byte)", 
             num_rngs, base_items_per_rng, remainder_bytes);
    println!("This ensures exactly {} total bits", total_target_bits);

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

    let total_trues: u64 = partial_sums.iter().sum();
    let total_bits: u64 = total_target_bits;
    let total_falses = total_bits - total_trues;
    let diff = total_trues as i64 - total_falses as i64;

    println!("Generated {} total bits: {} ones, {} zeros", 
             total_bits, total_trues, total_falses);
    println!("Ratio: {:.6} ones per bit", total_trues as f64 / total_bits as f64);

    if diff > 0 {
        println!("\x1b[1;32myes by {} votes\x1b[0m", diff);
    } else if diff < 0 {
        println!("\x1b[1;31mno by {} votes\x1b[0m", -diff);
    } else {
        println!("MIRACLE! It's a tie");
    }
}

fn fetch_qrandom_bytes(num_bytes: usize) -> Vec<u8> {
    let url = format!("https://qrandom.io/api/random/binary?bytes={}", num_bytes);
    let response = get(&url).expect("Failed to fetch from qrandom.io");
    let json_response: QRandomResponse = response.json().expect("Failed to parse JSON response");
    
    let binary_response = get(&json_response.binary_url).expect("Failed to fetch binary data");
    binary_response.bytes().expect("Failed to get bytes").to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test seed generation with SHA-256
    #[test]
    fn test_seed_generation() {
        let test_bytes = vec![42u8; 64]; // 64 bytes of test data
        let seeds = (0..2).into_par_iter()
            .map(|i| {
                let start = i * test_bytes.len() / 2;
                let end = (i + 1) * test_bytes.len() / 2;
                let slice = &test_bytes[start..end];
                let mut hasher = Sha256::new();
                hasher.update(slice);
                hasher.update(&i.to_be_bytes());
                hasher.finalize().into()
            })
            .collect::<Vec<[u8; 32]>>();
        
        assert_eq!(seeds.len(), 2);
        assert_ne!(seeds[0], seeds[1]); // Different seeds for different chunks
    }

    // Test parallel bit counting
    #[test]
    fn test_parallel_bit_counting() {
        let test_seeds = vec![[0u8; 32], [255u8; 32]]; // All 0s and all 1s
        
        let results: Vec<u64> = test_seeds.par_iter()
            .map(|&seed| {
                let mut rng = StdRng::from_seed(seed);
                (0..1000)
                    .map(|_| rng.random::<u8>())
                    .map(|byte| byte.count_ones() as u64)
                    .sum()
            })
            .collect();
        
        // First seed should produce mostly 0s (but not guaranteed)
        // Second seed should produce mostly 1s (but not guaranteed)
        assert!(results[0] < results[1]);
    }
}