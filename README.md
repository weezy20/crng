# qcoin - Quantum Coin Toss

A quantum-based binary choice maker that performs coin tosses using quantum random number generators. The application tries multiple quantum sources and uses a mix of cryptographically secure RNGs seeded with the quantum bytes obtained from a publicly available QRNG.

## Installation
You can build the project from source or install it directly from [crates.io](https://crates.io/crates/qcoin).

```bash
cargo install qcoin
```

## Usage

```bash
cargo run           # Single coin flip
cargo run 10        # 10 coin flips
cargo run 100       # 100 coin flips
```

### Command Line Options

| Flag | Description | Default | Notes |
|------|-------------|---------|-------|
| `<number>` | Number of coin flips | `1` | First positional argument |
| `-o, --out <file>` | Output file for quantum entropy | `qrandom.bytes` | Only saves when quantum sources succeed |

**Note**: If the specified output file already exists, the program will warn and use the default file instead.

## How it Works

### Entropy Source Priority

The application attempts to fetch truly random bytes from sources in this order:

1. **🔬 [ANU QRNG](https://qrng.anu.edu.au/)** - Australian National University's Quantum Random Number Generator
2. **🌐 [qrandom.io](https://qrandom.io/)** 
3. **🔒 Cryptographic SRNG** - Cryptographically secure random number generator (fallback)
4. **♻️ Saved Quantum Bytes** - Previously saved quantum entropy from `qrandom.bytes`

### Core Logic

The program implements a hybrid approach for multiple coin flips:

#### Single Flip (N=1)
- Uses quantum bytes directly to count 1s vs 0s
- Pure quantum randomness determines the outcome

#### Multiple Flips (N>1) 
- **N-1 flips**: Generated using a CSRNG seeded with quantum entropy
- **1 flip**: Uses the original quantum bytes directly
- **Parallel processing**: CSRNG flips are generated in parallel for performance

This approach ensures:
- At least one flip always uses pure quantum randomness
- Computational efficiency for large numbers of flips
- Deterministic results when using the same quantum seed

### Example Flow for 10 Flips
1. Fetch 1024 quantum bytes from ANU QRNG
2. Save quantum bytes to `qrandom.bytes`
3. Generate 9 flips (9 × 1024 = 9,216 bytes) using quantum-seeded CSRNG
4. Use the original 1024 quantum bytes for the 10th flip
5. Count total 1s vs 0s across all 10,240 bytes
6. Determine outcome: more 1s = YES, more 0s = NO

### Output
```
🎲 Quantum Coin Toss

📊 Flips: 10

🔍 Trying ANU QRNG...
✅ ANU QRNG: 1024 bytes
💾 Saved quantum entropy to file
🌱 Using quantum seed for 10 flips (9 CSRNG + 1 quantum)
⚡ Generating 9216 bytes from seeded CSRNG (9 flips)
✅ Generated 9216 bytes from CSRNG
🔬 Using quantum entropy directly for final flip

📈 Result: 40,960 ones, 40,944 zeros
🎯 Outcome: YES
```

## Dependencies

- `rand` - Cryptographic random number generation and seeding
- `reqwest` - HTTP client for quantum API requests  
- `serde` - JSON parsing for API responses
- `rayon` - Parallel processing for multiple flips
- `hex` - Hexadecimal encoding/decoding utilities
- `sha2` - SHA-2 cryptographic hash functions
