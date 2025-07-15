# crng

A binary choice maker based on quantum random number generator that fetches entropy from qrandom.io and uses that to generate 40 million bits to decide if result is a yes (more 1s) or a no (more 0s) or a tie.

## Usage

```bash
# Use default 1024 bytes
cargo run

# Specify number of bytes
cargo run 2048
cargo run 64
```

## Output

- Downloads quantum random bytes from qrandom.io
- Saves hex dump to `qrandom_bytes.hex`
- Seeds multiple RNGs for parallel bit generation
- Counts 1s vs 0s in 40 million generated bits
- Shows result: **yes** (more 1s) or **no** (more 0s)

## Dependencies

- `rand` - Random number generation
- `rayon` - Parallel processing
- `reqwest` - HTTP client
- `serde` - JSON parsing
