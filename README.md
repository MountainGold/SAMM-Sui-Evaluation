# SAMM-Sui-Evaluation

Paper link: https://arxiv.org/abs/2406.05568


## Folders
### sui (appears in further constructions):
The special version of sui. Modified from sui v1.10.0.
Changes:
1. Making sui-test-validator multithreaded.
2. Change the default maximal links of JSONrpsee.
Copied from https://github.com/MountainGold/sui-samm

### SAMM-evaluation:
Test code of SAMM.


## Prerequisites before running the evaluation (for linux)

Note that it doesn't matter which sui and sui-test-validator version you have installed. We will use the compiled files in this repo.

### download the sui files
git clone https://github.com/MountainGold/sui-samm sui

### Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

### Update Rust
rustup update stable

### Install sui prerequisites
sudo apt-get update
sudo apt-get install curl git-all cmake gcc libssl-dev pkg-config libclang-dev libpq-dev build-essential

### Build sui and sui-test-validator
cd sui
cargo build --bin sui-test-validator --release
cargo build --bin sui --release

### Then, turn to the SAMM-evaluation folder for further instructions
cd SAMM-evaluation
vim README.md


