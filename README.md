# HTTP Client in Rust

This project is a simple HTTP client written in Rust that downloads data from a glitchy server using range requests and computes the SHA-256 hash of the received data. It’s designed to handle server quirks, such as truncated responses, and ensures all data is fetched reliably.

## Project Structure
- `src/main.rs`: The main client code.
- `src/tests.rs`: Unit tests for key functionality.
- `Cargo.toml`: Rust project configuration.
- `buggy_server.py`: The Python server (assumed to be provided or part of the task).

## Prerequisites
- **Rust**: Install via [rustup](https://rustup.rs/) (version 1.56+ for 2021 edition).
- **Python**: Version 3.x for the server.
- A terminal to run commands.

## Setup
1. **Clone the Repository** (if applicable):
   ```bash
   git clone <repository-url>
   cd http_client
   ```

2. **Building the Client**
Build the Rust client using Cargo:
    ```bash
    cargo build
    ```
3. **Running the Server**
    ```bash
    python3 buggy_server.py
    ```
- Runs on 127.0.0.1:8080 by default.
- Outputs the SHA-256 hash of its data for verification.

4. **Running the Client**
   Run the client to download from the server:
    ```bash
   cargo run -- 127.0.0.1 8080
    ```
- Arguments: `<address>` `<port>`.
- Outputs progress and the final SHA-256 hash.

5. **Testing the Client**
    ```bash
    cargo test
   ```
- Tests in `src/tests.rs` verify header parsing and chunk extraction.
- Expected output:
    ```bash
    running 3 tests
    test tests::test_get_content_length ... ok
    test tests::test_download_chunk_full_response ... ok
    test tests::test_download_chunk_empty_response ... ok
    ```

## Notes
- The client requests the full remaining data in each chunk, adapting to the server’s truncation (e.g., limiting responses to ≥64KB).
- The SHA-256 hash should match the server’s output if all data is downloaded correctly.
- Tests mock I/O to avoid network calls, focusing on parsing logic.

## Troubleshooting
- Server Not Running: Ensure buggy_server.py is active before running the client.
- Port Conflict: Change the port in both server and client commands if 8080 is in use.
- Rust Errors: Run cargo clean && cargo build to reset the build state


    

