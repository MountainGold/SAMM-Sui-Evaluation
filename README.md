# SAMM-Sui-Evaluation

## Folders
### sui:
The special version of sui. Modified from sui v1.10.0.
Changes:
1. Making sui-test-validator multithreaded.
2. Change the default maximal links of JSONrpsee.
Copied from https://github.com/MountainGold/sui-samm

### SAMM-evaluation:
Test code of SAMM.


## Prerequisites before running the evaluation (for linux)

### Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

### Update Rust
rustup update stable

### Install sui prerequisites
sudo apt-get update
sudo apt-get install curl git-all cmake gcc libssl-dev pkg-config libclang-dev libpq-dev build-essential

### Build sui and sui-test-validators
cd sui
cargo build --bin sui-test-validator sui --release




