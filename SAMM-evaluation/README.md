## Folders:

### omniswap
Smart contract code of OmniSwap
More info in https://github.com/OmniBTC/Sui-AMM-swap

### samm
Smart contract code of SAMM

### samm-boost
Smart contract code of heavier SAMM

### test omniswap
Test code of OmniSwap

### test-samm
Test code of SAMM

### test-heavier-samm
Test code of heavier samm

### test-token-transfer
Test code of token transfers

## Instruction of test (SAMM test as the example)

### Enter the test folder 
cd test-samm

### Create the ledger folder
mkdir suilog

### (Optional) mount the folder with RAM
sudo mount -t tmpfs -o size=128G tmpfs suilog

### Run the test
cargo run

Make sure ports 9000 and 9123 are available (especially, check whether existing sui-test-validators are running).

After running the test, you need to input some parameters, including:
num_clients: the number of trader clients (suggestion: 100)
min_tps: the minimal expected TPS
max_tps: the maximal expected TPS
tps_gap: the increase of TPS after a test
num_repeat: the repeated time of a fixed TPS
num_groups: the number of test groups with different shards
num_shards: the number of shards in each group

In each group, the experiment starts from the minimal expected TPS, repeating for num_repeat times. In each repetition, the code cleans historical data in the suilog folder and starts a new sui-test-validator. Once trader clients begin sending transactions, the system undergoes a warmup period of 500 seconds, followed by a testing period of 100 seconds (modifiable in src/main.rs).

After completing all repetitions, if there are too many failures or the latency is excessively high, the code proceeds to the next group. Otherwise, the code increments the TPS and tests again. If the latency remains very low, the minimal expected TPS is increased for subsequent groups.

The results of each execution are stored in a folder named by the experiment's start time.