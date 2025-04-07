use std::io::{self, Cursor, Read, Write};
use crate::{get_content_length, process_chunks, read_response, download_chunk};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

struct MockStream {
    input: Cursor<Vec<u8>>,
    output: Vec<u8>,
    max_read: Option<usize>, // Optional cap on bytes read per call to simulate truncation
}

impl MockStream {
    fn new(response: &str) -> Self {
        MockStream {
            input: Cursor::new(response.as_bytes().to_vec()),
            output: Vec::new(),
            max_read: None,
        }
    }

    fn with_truncation(response: &str, max_read: usize) -> Self {
        MockStream {
            input: Cursor::new(response.as_bytes().to_vec()),
            output: Vec::new(),
            max_read: Some(max_read),
        }
    }
}

impl Read for MockStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(max) = self.max_read {
            let capped_len = buf.len().min(max);
            self.input.read(&mut buf[..capped_len])
        } else {
            self.input.read(buf)
        }
    }
}

impl Write for MockStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.output.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_get_content_length() -> io::Result<()> {
    let response = "HTTP/1.1 200 OK\r\nContent-Length: 42\r\n\r\nsome data";
    let mut stream = MockStream::new(response);
    let result = get_content_length_with_stream(&mut stream, "test")?;
    assert_eq!(result, 42);

    let response = "HTTP/1.1 200 OK\r\n\r\nno length";
    let mut stream = MockStream::new(response);
    let result = get_content_length_with_stream(&mut stream, "test");
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);

    Ok(())
}

#[test]
fn test_download_chunk_full_response() -> io::Result<()> {
    let response = "HTTP/1.1 206 Partial Content\r\nContent-Length: 5\r\n\r\nhello";
    let mut stream = MockStream::new(response);
    let chunk = download_chunk_with_stream(&mut stream, "test", 0, 5)?;
    assert_eq!(chunk, b"hello");
    Ok(())
}

#[test]
fn test_download_chunk_empty_response() -> io::Result<()> {
    let response = "HTTP/1.1 206 Partial Content\r\nContent-Length: 0\r\n\r\n";
    let mut stream = MockStream::new(response);
    let chunk = download_chunk_with_stream(&mut stream, "test", 0, 5)?;
    assert!(chunk.is_empty());
    Ok(())
}

#[test]
fn test_process_chunks() -> io::Result<()> {
    let mut hasher = Sha256::new();
    let mut bytes_hashed = 0;
    let mut chunk_buffer = BTreeMap::new();
    let total_size = 10;

    // Normal case: contiguous chunks
    chunk_buffer.insert(5, vec![5, 6, 7, 8, 9]);
    chunk_buffer.insert(0, vec![0, 1, 2, 3, 4]);
    process_chunks(&mut hasher, &mut bytes_hashed, &mut chunk_buffer, total_size)?;
    assert_eq!(bytes_hashed, 10);
    assert!(chunk_buffer.is_empty());

    // Excess data case
    chunk_buffer.insert(10, vec![10]);
    let result = process_chunks(&mut hasher, &mut bytes_hashed, &mut chunk_buffer, total_size);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);

    Ok(())
}

#[test]
fn test_multi_request_chunk_processing() -> io::Result<()> {
    let mut hasher = Sha256::new();
    let mut bytes_hashed = 0;
    let mut chunk_buffer = BTreeMap::new();
    let total_size = 10;

    // Simulate thread fetching range 0-9 in multiple truncated requests
    let response1 = "HTTP/1.1 206 Partial Content\r\nContent-Length: 5\r\n\r\n01234";
    let mut stream1 = MockStream::with_truncation(response1, 5);
    let chunk1 = download_chunk_with_stream(&mut stream1, "test", 0, 10)?;
    chunk_buffer.insert(0, chunk1); // First 5 bytes

    let response2 = "HTTP/1.1 206 Partial Content\r\nContent-Length: 5\r\n\r\n56789";
    let mut stream2 = MockStream::new(response2); // No truncation for simplicity
    let chunk2 = download_chunk_with_stream(&mut stream2, "test", 5, 10)?;
    chunk_buffer.insert(5, chunk2); // Next 5 bytes

    process_chunks(&mut hasher, &mut bytes_hashed, &mut chunk_buffer, total_size)?;
    assert_eq!(bytes_hashed, 10);
    assert!(chunk_buffer.is_empty());

    // Simulate gap (fallback needed)
    let mut hasher = Sha256::new();
    let mut bytes_hashed = 0;
    let mut chunk_buffer = BTreeMap::new();
    chunk_buffer.insert(0, vec![0, 1, 2]); // Thread got only 0-2
    process_chunks(&mut hasher, &mut bytes_hashed, &mut chunk_buffer, total_size)?;
    assert_eq!(bytes_hashed, 3); // Gap at 3-9 remains
    assert!(chunk_buffer.is_empty());

    // Fallback simulation
    let response_fallback = "HTTP/1.1 206 Partial Content\r\nContent-Length: 7\r\n\r\n3456789";
    let mut stream_fallback = MockStream::new(response_fallback);
    let chunk_fallback = download_chunk_with_stream(&mut stream_fallback, "test", 3, 10)?;
    chunk_buffer.insert(3, chunk_fallback);
    process_chunks(&mut hasher, &mut bytes_hashed, &mut chunk_buffer, total_size)?;
    assert_eq!(bytes_hashed, 10);
    assert!(chunk_buffer.is_empty());

    Ok(())
}

fn get_content_length_with_stream(stream: &mut MockStream, server_addr: &str) -> io::Result<usize> {
    let request = format!("GET / HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", server_addr);
    stream.write_all(request.as_bytes())?;
    let (headers, _) = read_response(stream)?;
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

fn download_chunk_with_stream(stream: &mut MockStream, server_addr: &str, start: usize, end: usize) -> io::Result<Vec<u8>> {
    let request = format!(
        "GET / HTTP/1.1\r\nHost: {}\r\nRange: bytes={}-{}\r\nConnection: close\r\n\r\n",
        server_addr, start, end
    );
    stream.write_all(request.as_bytes())?;
    let (_, body) = read_response(stream)?;
    Ok(body)
}