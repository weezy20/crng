# qcoin - Quantum Coin Toss

A quantum-based binary choice maker that performs coin tosses using quantum random number generators. The application tries multiple quantum sources and uses a mix of cryptographically secure RNGs seeded with the quantum bytes obtained from a publicly available QRNG.

## Installation
You can build the project from source or install it directly from [crates.io](https://crates.io/crates/qcoin).

```bash
cargo install qcoin
```

## Usage

```bash
qcoin                           # Single coin flip
qcoin -n 10                     # 10 coin flips
qcoin --number 100              # 100 coin flips
qcoin -s entropy.hex            # Use hex string from entropy.hex as entropy source
qcoin --hex "abc123"            # Use hex string directly as entropy source
qcoin --hex "0xff"              # Use hex string with 0x prefix
qcoin -n 5 --hex "abc123"       # 5 flips using hex string
qcoin --hex "ff" -o saved.hex   # Use hex and save to custom file
```

### Command Line Options

| Flag | Description | Default | Notes |
|------|-------------|---------|-------|
| `-n, --number <flips>` | Number of coin flips | `1` | Must be greater than 0 |
| `-o, --output <file>` | Output file for quantum entropy | `qrandom.bytes` | Saves quantum or hex entropy |
| `-s, --source <file>` | Use file as entropy source | None | Supports hex strings or binary data |
| `--hex <string>` | Use hex string directly as entropy source | None | Supports 0x prefix |

## How it Works

### Coin Flip Logic

Counts 1-bits vs 0-bits in entropy bytes:
- **More 1-bits** → **YES** ✅
- **More 0-bits** → **NO** ❌

**Single flip**: Uses entropy bytes directly  
**Multiple flips**: `N-1` CSRNG-generated with random bytes as its seed + 1 direct entropy flip

### Entropy Sources

1. [ANU QRNG](https://qrng.anu.edu.au/) - Quantum random number generator
2. [qrandom.io](https://qrandom.io/) - Alternative quantum source  
3. User input `--hex <entropy>` or `-s/--source <file>`
4. Cryptographic SRNG - Fallback
5. Saved quantum bytes from `qrandom.bytes`
