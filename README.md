# HTTP Client in Rust

This project is a simple HTTP client written in Rust that downloads data from a glitchy server using range requests and computes the SHA-256 hash of the received data. It’s designed to handle server quirks, such as truncated responses, and ensures all data is fetched reliably.

## Project Structure
- `src/main.rs`: The main client code with multi-threaded downloading and hashing logic.
- `src/tests.rs`: Unit tests for key functionality.
- `Cargo.toml`: Rust project configuration with `sha2` dependency.
- `buggy_server.py`: The Python server

## Prerequisites
- **Rust**: Install via [rustup](https://rustup.rs/) (version 1.56+ for 2021 edition).
- **Python**: Version 3.x for the server.
- A terminal to run commands.

## Setup
Clone the Repository (if applicable):
```bash
git clone https://github.com/mikolajed/HTTP-client-in-Rust.git
cd HTTP-client-in-Rust
```

## Building the Client
Build the Rust client using Cargo:
```bash
cargo build
```
## Running the Server
```bash
python3 buggy_server.py
```
- Runs on 127.0.0.1:8080 by default.
- Outputs the SHA-256 hash of its data for verification.

## Running the Client
Run the client to download from the server:
```bash
cargo run -- 127.0.0.1 8080 4
```
- **Arguments**:
  - `<address>`: Server IP (e.g., `127.0.0.1`).
  - `<port>`: Server port (e.g., `8080`).
  - `[num_threads]`: Optional number of concurrent threads (e.g., `4`).
- **Outputs**: progress and the final SHA-256 hash.

## Client Logic
1. **Content Length Fetch**: Sends a GET request to retrieve the total size via Content-Length.
2. **Parallel Downloads**: Divides the data into num_threads chunks. Each thread fetches its range (Range: bytes=start-end) with multiple requests if truncated, storing chunks in a `BTreeMap`.
3. **Handling Truncation**: After threads finish, the main thread sequentially fetches any missing ranges (gaps) until all total_size bytes are hashed.
4. **Fallback Loop**: After threads finish, the main thread sequentially fetches any missing ranges (gaps) until all `total_size` bytes are hashed.
5. **Hashing**: Processes chunks in order, computing the SHA-256 hash incrementally, ensuring `bytes_hashed` matches `total_size`.
7. **Example**: For `total_size = 524288` and `num_threads = 4`, each thread requests `~131072` bytes. 

## Advantages Over Naive Sequential Approach
Compared to a naive client that downloads the entire file sequentially and computes the SHA-256 hash once:
1. **Concurrency**: Multi-threading speeds up downloads by fetching ranges in parallel, reducing total time for large files (e.g., 524KB with 4 threads vs. one sequential request).
2. **Incremental Hashing**: Computes the hash as chunks arrive, avoiding the need to store the full file in memory before hashing—ideal for memory-constrained environments.
3. **Flexibility**: Configurable `num_threads` optimizes performance based on network/server conditions, while a sequential client is stuck with one request.

## Testing the Client
```bash
cargo test
```
- Tests in `src/tests.rs` verify header parsing and chunk extraction.
- Expected output:
```bash
running 5 tests
test tests::test_download_chunk_empty_response ... ok
test tests::test_download_chunk_full_response ... ok
test tests::test_get_content_length ... ok
test tests::test_process_chunks ... ok
test tests::test_multi_request_chunk_processing ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Testing Approach
1. **Location**: Tests are in src/tests.rs, included via #[cfg(test)] mod tests; in main.rs.
2. **Mocking**: Uses a MockStream struct implementing Read and Write to simulate HTTP responses without network calls.
3. **Tests**:
   - `test_get_content_length`: Verifies parsing Content-Length and errors on missing headers.
   - `test_download_chunk_full_response`: Ensures full partial responses (e.g., "hello") are extracted.
   - `test_download_chunk_empty_response`: Confirms empty responses are handled.
   - `test_process_chunks`: Tests ordered chunk hashing and excess data rejection.
   - `test_multi_request_chunk_processing`: Simulates multi-request chunk fetching and fallback for gaps
4. **Coverage**: Focuses on parsing, chunk processing, and error handling. Multi-threading and network I/O are tested manually with the server.

## Notes
- **Server Quirks**: The client adapts to truncation (e.g., responses capped at 64KB) and delays by retrying missing ranges.
- **Hash Verification**: The final SHA-256 hash should match the server’s output if all total_size bytes are fetched.
- **Thread Safety**: Uses `Arc<Mutex<BTreeMap>>` to safely share chunks across threads.

## Troubleshooting
- Server Not Running: Start buggy_server.py before the client.
- Port Conflict: Change the port in both server and client commands (e.g., 8081).
- Test Failure: Check src/tests.rs for mismatches; run cargo test -- --nocapture for detailed output.
- Rust Errors: Use cargo clean && cargo build to reset the build state.




    

