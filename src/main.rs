use std::io::{self, Read, Write};
use std::net::TcpStream;
use sha2::{Digest, Sha256};
use std::env;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::thread;

#[cfg(test)]
mod tests;

pub fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 || args.len() > 4 {
        eprintln!("Usage: {} <address> <port> [num_threads]", args[0]);
        eprintln!("Example: {} 127.0.0.1 8080 4", args[0]);
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid number of arguments"));
    }

    let address = &args[1];
    let port: u16 = args[2].parse().map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidInput, "Port must be a number between 0 and 65535")
    })?;
    let server_addr = format!("{}:{}", address, port);

    let num_threads = if args.len() == 4 {
        args[3].parse::<usize>().map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "Number of threads must be a positive integer")
        })?
    } else {
        1
    };
    if num_threads == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Number of threads must be at least 1"));
    }

    let total_size = get_content_length(&server_addr)?;
    println!("Total size to download: {} bytes", total_size);
    println!("Using {} threads", num_threads);

    let mut hasher = Sha256::new();
    let mut bytes_hashed = 0;
    let chunk_buffer = Arc::new(Mutex::new(BTreeMap::new()));
    let chunk_size = (total_size + num_threads - 1) / num_threads;

    let mut handles = Vec::new();
    for i in 0..num_threads {
        let start = i * chunk_size;
        let end = if i == num_threads - 1 { total_size - 1 } else { (i + 1) * chunk_size - 1 };
        if start >= total_size {
            break;
        }

        let server_addr = server_addr.clone();
        let chunk_buffer = Arc::clone(&chunk_buffer);
        let handle = thread::spawn(move || {
            let mut current_start = start;
            let range_end = end; // Final byte to fetch
            while current_start <= range_end {
                println!("Thread {} requesting range: bytes={}-{}", i, current_start, range_end);
                match download_chunk(&server_addr, current_start, range_end + 1) {
                    Ok(chunk) => {
                        let mut buffer = chunk_buffer.lock().unwrap();
                        if chunk.is_empty() {
                            eprintln!("Thread {} received empty chunk for {}-{}, advancing 1 byte", i, current_start, range_end);
                            buffer.insert(current_start, vec![0]);
                            current_start += 1;
                        } else {
                            let chunk_size = chunk.len();
                            buffer.insert(current_start, chunk);
                            current_start += chunk_size;
                            println!("Thread {} fetched {} bytes, now at {}", i, chunk_size, current_start);
                        }
                    }
                    Err(e) => {
                        eprintln!("Thread {} failed to download {}-{}: {}, advancing 1 byte", i, current_start, range_end, e);
                        let mut buffer = chunk_buffer.lock().unwrap();
                        buffer.insert(current_start, vec![0]);
                        current_start += 1;
                    }
                }
            }
            println!("Thread {} completed range {}-{}", i, start, range_end);
        });
        handles.push(handle);
    }

    for handle in handles {
        match handle.join() {
            Ok(()) => println!("Thread joined successfully"),
            Err(e) => eprintln!("Thread panicked: {:?}", e),
        }
    }

    let mut chunk_buffer = chunk_buffer.lock().unwrap();
    while bytes_hashed < total_size {
        process_chunks(&mut hasher, &mut bytes_hashed, &mut chunk_buffer, total_size)?;

        if bytes_hashed < total_size {
            let start = bytes_hashed;
            let end = total_size - 1;
            println!("Main thread fetching missing range: bytes={}-{}", start, end);
            match download_chunk(&server_addr, start, end + 1) {
                Ok(chunk) => {
                    if chunk.is_empty() {
                        eprintln!("Main thread received empty chunk for {}-{}, skipping 1 byte", start, end);
                        chunk_buffer.insert(start, vec![0]);
                    } else {
                        chunk_buffer.insert(start, chunk);
                    }
                }
                Err(e) => {
                    eprintln!("Main thread failed to download {}-{}: {}, skipping 1 byte", start, end, e);
                    chunk_buffer.insert(start, vec![0]);
                }
            }
        }
    }

    let hash = hasher.finalize();
    println!("Hashed {} bytes", bytes_hashed);
    println!("Final message - SHA-256 hash of the downloaded data: {:x}", hash);

    if bytes_hashed != total_size {
        eprintln!("Warning: Hashed {} bytes, expected {}", bytes_hashed, total_size);
    }

    Ok(())
}

pub fn get_content_length(server_addr: &str) -> io::Result<usize> {
    let mut stream = TcpStream::connect(server_addr)?;
    let request = format!("GET / HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", server_addr);
    stream.write_all(request.as_bytes())?;

    let (headers, _) = read_response(&mut stream)?;
    let header_str = std::str::from_utf8(&headers).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, format!("Invalid UTF-8 in headers: {}", e))
    })?;

    for line in header_str.lines() {
        if line.to_lowercase().starts_with("content-length:") {
            return Ok(line.split(':').nth(1).unwrap().trim().parse().unwrap());
        }
    }
    Err(io::Error::new(io::ErrorKind::NotFound, "Content-Length not found"))
}

pub fn process_chunks(
    hasher: &mut Sha256,
    bytes_hashed: &mut usize,
    chunk_buffer: &mut BTreeMap<usize, Vec<u8>>,
    total_size: usize,
) -> io::Result<()> {
    loop {
        let first_entry = chunk_buffer.first_key_value().map(|(&k, v)| (k, v.clone()));
        match first_entry {
            Some((start, chunk)) => {
                if start < *bytes_hashed {
                    chunk_buffer.remove(&start); // Overlap, discard
                } else if start == *bytes_hashed {
                    if *bytes_hashed + chunk.len() > total_size {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Chunk exceeds total size"
                        ));
                    }
                    hasher.update(&chunk);
                    *bytes_hashed += chunk.len();
                    chunk_buffer.remove(&start);
                    println!("Hashed chunk starting at {}, size {}, now at {}", start, chunk.len(), *bytes_hashed);
                } else {
                    break; // Gap, wait for missing chunk
                }
            }
            None => break,
        }
    }

    if *bytes_hashed > total_size {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Hashed bytes exceed total size"));
    }

    Ok(())
}

pub fn read_response<R: Read + Write>(stream: &mut R) -> io::Result<(Vec<u8>, Vec<u8>)> {
    let mut buffer = Vec::new();
    let mut headers_complete = false;
    let mut header_end = 0;

    while !headers_complete {
        let mut temp = [0; 4096];
        let bytes_read = stream.read(&mut temp)?;
        if bytes_read == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Connection closed prematurely"));
        }
        buffer.extend_from_slice(&temp[..bytes_read]);
        if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = pos + 4;
            headers_complete = true;
        }
    }

    let headers = buffer[..header_end].to_vec();
    let mut body = buffer[header_end..].to_vec();

    let header_str = std::str::from_utf8(&headers).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, format!("Invalid UTF-8 in headers: {}", e))
    })?;
    let mut content_length = None;
    for line in header_str.lines() {
        if line.to_lowercase().starts_with("content-length:") {
            content_length = Some(line.split(':').nth(1).unwrap().trim().parse::<usize>().unwrap());
            break;
        }
    }

    if let Some(len) = content_length {
        let mut remaining = len - body.len();
        while remaining > 0 {
            let mut temp = vec![0; remaining.min(4096)];
            match stream.read(&mut temp) {
                Ok(0) => break,
                Ok(n) => {
                    body.extend_from_slice(&temp[..n]);
                    remaining -= n;
                }
                Err(e) => return Err(e),
            }
        }
    }

    Ok((headers, body))
}

pub fn download_chunk(server_addr: &str, start: usize, end: usize) -> io::Result<Vec<u8>> {
    let mut stream = TcpStream::connect(server_addr)?;
    let request = format!(
        "GET / HTTP/1.1\r\nHost: {}\r\nRange: bytes={}-{}\r\nConnection: close\r\n\r\n",
        server_addr, start, end
    );
    stream.write_all(request.as_bytes())?;

    let (_, body) = read_response(&mut stream)?;
    Ok(body)
}