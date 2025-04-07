use std::io::{self, Read, Write};
use std::net::TcpStream;
use sha2::{Digest, Sha256};
use std::env;

#[cfg(test)]
mod tests;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <address> <port>", args[0]);
        eprintln!("Example: {} 127.0.0.1 8080", args[0]);
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Missing address or port"));
    }

    let address = &args[1];
    let port: u16 = args[2].parse().map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidInput, "Port must be a number between 0 and 65535")
    })?;
    let server_addr = format!("{}:{}", address, port);

    let total_size = get_content_length(&server_addr)?;
    println!("Total size to download: {} bytes", total_size);

    let mut full_data = Vec::new();
    let mut bytes_downloaded = 0;

    while bytes_downloaded < total_size {
        let start = bytes_downloaded;
        let end = total_size - 1;
        println!("Requesting range: bytes={}-{}", start, end);

        match download_chunk(&server_addr, start, end + 1) {
            Ok(chunk) => {
                if chunk.is_empty() {
                    eprintln!("Received empty chunk for {}-{}, skipping 1 byte", start, end);
                    bytes_downloaded += 1;
                    continue;
                }
                full_data.extend_from_slice(&chunk);
                bytes_downloaded += chunk.len();
                println!("Received {} bytes, total now: {}", chunk.len(), bytes_downloaded);
            }
            Err(e) => {
                eprintln!("Failed to download {}-{}: {}", start, end, e);
                bytes_downloaded += 1;
            }
        }
    }

    let mut hasher = Sha256::new();
    hasher.update(&full_data);
    let hash = hasher.finalize();
    println!("Downloaded {} bytes", full_data.len());
    println!("Final message - SHA-256 hash of the downloaded data: {:x}", hash);

    Ok(())
}

fn get_content_length(server_addr: &str) -> io::Result<usize> {
    let mut stream = TcpStream::connect(server_addr)?;
    let request = format!("GET / HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", server_addr);
    stream.write_all(request.as_bytes())?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;

    let header_end = response.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4).unwrap_or(0);
    let headers = &response[..header_end];
    let header_str = std::str::from_utf8(headers).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, format!("Invalid UTF-8 in headers: {}", e))
    })?;

    for line in header_str.lines() {
        if line.to_lowercase().starts_with("content-length:") {
            return Ok(line.split(':').nth(1).unwrap().trim().parse().unwrap());
        }
    }
    Err(io::Error::new(io::ErrorKind::NotFound, "Content-Length not found"))
}

fn download_chunk(server_addr: &str, start: usize, end: usize) -> io::Result<Vec<u8>> {
    let mut stream = TcpStream::connect(server_addr)?;
    let request = format!(
        "GET / HTTP/1.1\r\nHost: {}\r\nRange: bytes={}-{}\r\nConnection: close\r\n\r\n",
        server_addr, start, end
    );
    stream.write_all(request.as_bytes())?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;

    let body_start = response.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4).unwrap_or(0);
    let body = response[body_start..].to_vec();

    Ok(body)
}