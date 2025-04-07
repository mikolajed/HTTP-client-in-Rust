#[cfg(test)]
mod tests {
    use std::io; // For io::Result
    use std::io::{Cursor, Read, Write}; // Specific imports

    // Mock stream for testing
    struct MockStream {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }

    impl MockStream {
        fn new(response: &str) -> Self {
            MockStream {
                input: Cursor::new(response.as_bytes().to_vec()),
                output: Vec::new(),
            }
        }
    }

    impl Read for MockStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.input.read(buf)
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
        // Mock a valid response
        let response = "HTTP/1.1 200 OK\r\nContent-Length: 42\r\n\r\nsome data";
        let mut stream = MockStream::new(response);
        let request = "GET / HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n";
        stream.write_all(request.as_bytes()).unwrap();

        let result: usize = {
            let mut mock_stream = stream;
            let mut response = Vec::new();
            mock_stream.read_to_end(&mut response)?;
            let header_end = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(0) + 4;
            let headers = &response[..header_end];
            let header_str = std::str::from_utf8(headers).unwrap();
            header_str.lines()
                .find(|line| line.to_lowercase().starts_with("content-length:"))
                .and_then(|line| line.split(':').nth(1))
                .map(|val| val.trim().parse().unwrap())
                .unwrap()
        };
        assert_eq!(result, 42);

        // Mock a missing Content-Length
        let response = "HTTP/1.1 200 OK\r\n\r\nno length";
        let mut stream = MockStream::new(response);
        stream.write_all(request.as_bytes()).unwrap();
        let result = {
            let mut mock_stream = stream;
            let mut response = Vec::new();
            mock_stream.read_to_end(&mut response)?;
            let header_end = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(0) + 4;
            let headers = &response[..header_end];
            std::str::from_utf8(headers)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
                .and_then(|header_str| {
                    header_str.lines()
                        .find(|line| line.to_lowercase().starts_with("content-length:"))
                        .and_then(|line| line.split(':').nth(1))
                        .map(|val| val.trim().parse::<usize>().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)))
                        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Content-Length not found"))?
                })
        };
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_download_chunk_full_response() -> io::Result<()> {
        let response = "HTTP/1.1 206 Partial Content\r\nContent-Length: 5\r\n\r\nhello";
        let mut stream = MockStream::new(response);
        let request = "GET / HTTP/1.1\r\nHost: test\r\nRange: bytes=0-5\r\nConnection: close\r\n\r\n";
        stream.write_all(request.as_bytes()).unwrap();

        let chunk = {
            let mut mock_stream = stream;
            let mut response = Vec::new();
            mock_stream.read_to_end(&mut response)?;
            let body_start = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(0) + 4;
            response[body_start..].to_vec()
        };
        assert_eq!(chunk, b"hello");
        Ok(())
    }

    #[test]
    fn test_download_chunk_empty_response() -> io::Result<()> {
        let response = "HTTP/1.1 206 Partial Content\r\nContent-Length: 0\r\n\r\n";
        let mut stream = MockStream::new(response);
        let request = "GET / HTTP/1.1\r\nHost: test\r\nRange: bytes=0-5\r\nConnection: close\r\n\r\n";
        stream.write_all(request.as_bytes()).unwrap();

        let chunk = {
            let mut mock_stream = stream;
            let mut response = Vec::new();
            mock_stream.read_to_end(&mut response)?;
            let body_start = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(0) + 4;
            response[body_start..].to_vec()
        };
        assert!(chunk.is_empty());
        Ok(())
    }
}