//! File-backed buffering for the homework's byte-at-a-time copying API.
//!
//! Call [`MyBufWriter::close`] or [`MyBufWriter::flush`] explicitly to observe
//! write errors. Drop attempts a final flush but cannot report failures; pending
//! bytes can be lost. Flushing does not guarantee crash durability.
//!
//! ```
//! use hw::{MyBufReader, MyBufWriter};
//!
//! # fn main() -> std::io::Result<()> {
//! # let directory = std::env::temp_dir().join(format!("hw-doc-{}", std::process::id()));
//! # std::fs::create_dir(&directory)?;
//! let path = directory.join("example.bin");
//! let mut writer = MyBufWriter::create(&path)?;
//! writer.write_buffered(b"abc")?;
//! writer.close()?;
//!
//! let mut reader = MyBufReader::open(&path)?;
//! assert_eq!(reader.read_byte()?, Some(b'a'));
//! # drop(reader);
//! # std::fs::remove_file(path)?;
//! # std::fs::remove_dir(directory)?;
//! # Ok(())
//! # }
//! ```

use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

/// Maximum number of pending bytes in each application buffer.
pub const BUFFER_SIZE: usize = 64 * 1024;

// -----------------------------------------------------------------------------
// MyBufReader
// -----------------------------------------------------------------------------

/// Reads one byte at a time while refilling a 64 KiB application buffer.
#[derive(Debug)]
pub struct MyBufReader {
    file: File,
    buffer: Vec<u8>,
    position: usize,
    filled: usize,
}

impl MyBufReader {
    /// Opens a file for sequential reading; returns an error if it cannot be opened.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            file: File::open(path)?,
            buffer: vec![0; BUFFER_SIZE],
            position: 0,
            filled: 0,
        })
    }

    /// Returns the next byte, `None` at EOF, or a file read error.
    pub fn read_byte(&mut self) -> io::Result<Option<u8>> {
        if self.position == self.filled {
            self.filled = loop {
                match self.file.read(&mut self.buffer) {
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    result => break result?,
                }
            };
            self.position = 0;
            if self.filled == 0 {
                return Ok(None);
            }
        }

        let byte = self.buffer[self.position];
        self.position += 1;
        Ok(Some(byte))
    }
}

// -----------------------------------------------------------------------------
// MyBufWriter
// -----------------------------------------------------------------------------

/// Collects writes in a 64 KiB application buffer before writing to a file.
///
/// Call `close` or `flush` explicitly: Drop ignores write errors and can lose data.
#[derive(Debug)]
pub struct MyBufWriter {
    file: File,
    buffer: Vec<u8>,
}

// Keep only the unwritten suffix after a short write or an I/O error.
fn write_pending(output: &mut impl Write, pending: &mut Vec<u8>) -> io::Result<()> {
    let mut written = 0;
    let result = loop {
        if written == pending.len() {
            break Ok(());
        }
        match output.write(&pending[written..]) {
            Ok(0) => {
                break Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "failed to write buffer",
                ));
            }
            Ok(count) => written += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => break Err(error),
        }
    };
    pending.drain(..written);
    result
}

impl MyBufWriter {
    /// Creates or truncates the output file; returns an error if creation fails.
    pub fn create(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            file: File::create(path)?,
            buffer: Vec::with_capacity(BUFFER_SIZE),
        })
    }

    /// Buffers the supplied bytes; may return an I/O error while draining a full buffer.
    /// On error, some bytes may already have been accepted or written, and this API
    /// does not return that count. Do not replay the entire slice on this writer;
    /// abandon the transfer or restart it using a fresh destination.
    pub fn write_buffered(&mut self, mut data: &[u8]) -> io::Result<()> {
        while !data.is_empty() {
            if self.buffer.len() == BUFFER_SIZE {
                self.flush()?;
            }
            let count = data.len().min(BUFFER_SIZE - self.buffer.len());
            self.buffer.extend_from_slice(&data[..count]);
            data = &data[count..];
            if self.buffer.len() == BUFFER_SIZE {
                self.flush()?;
            }
        }
        Ok(())
    }

    /// Drains pending bytes to the file; does not guarantee durable storage.
    /// Retains any unwritten suffix if a write fails, so another flush can retry it.
    pub fn flush(&mut self) -> io::Result<()> {
        write_pending(&mut self.file, &mut self.buffer)
    }

    /// Flushes pending bytes and propagates write errors before dropping the writer.
    pub fn close(mut self) -> io::Result<()> {
        self.flush()
    }
}

impl Drop for MyBufWriter {
    fn drop(&mut self) {
        // Only this destructor fallback ignores file-write errors: Drop cannot
        // return them. Pending data may be lost; explicit flush/close is required
        // for checked completion. Tested with a read-only file and pending bytes.
        #[allow(clippy::let_underscore_must_use)]
        let _ = self.flush();
    }
}

// -----------------------------------------------------------------------------
// Unbuffered copy
// -----------------------------------------------------------------------------

/// Copies one byte at a time using direct file I/O and returns the byte count.
///
/// Creates or truncates `output`; input and output must name distinct files.
/// Returns open, read, write, or flush errors; errors can leave partial output.
pub fn copy_slow(input: impl AsRef<Path>, output: impl AsRef<Path>) -> io::Result<u64> {
    let mut input = File::open(input)?;
    let mut output = File::create(output)?;

    let mut copied = 0;
    let mut byte = [0u8; 1];

    loop {
        let n = input.read(&mut byte)?;
        if n == 0 {
            break;
        }

        output.write_all(&byte[..n])?;
        copied += n as u64;
    }

    output.flush()?;

    Ok(copied)
}

// -----------------------------------------------------------------------------
// Buffered copy
// -----------------------------------------------------------------------------
// The assignment keeps this byte-at-a-time loop unchanged: buffering belongs in
// MyBufReader and MyBufWriter, not in the copy algorithm.

/// Copies through the custom buffers and returns the byte count after closing.
///
/// Creates or truncates `output`; input and output must name distinct files.
/// Returns open, read, write, or close errors; errors can leave partial output.
pub fn copy_fast(input: impl AsRef<Path>, output: impl AsRef<Path>) -> io::Result<u64> {
    let mut reader = MyBufReader::open(input)?;
    let mut writer = MyBufWriter::create(output)?;

    let mut copied = 0;

    while let Some(byte) = reader.read_byte()? {
        writer.write_buffered(&[byte])?;
        copied += 1;
    }

    writer.close()?;

    Ok(copied)
}

/// Number of bytes in a generated demonstration record.
pub const RECORD_SIZE: usize = 10;

/// Generates ten bytes with mathematical value `(index + offset) % 251`.
///
/// Every `usize` index is supported, without machine-word wrapping.
pub fn make_record(index: usize) -> [u8; RECORD_SIZE] {
    let mut record = [0u8; RECORD_SIZE];

    (0..RECORD_SIZE).for_each(|i| {
        // Reduce first: the sum is at most 250 + 9, and the result fits u8.
        record[i] = ((index % 251 + i) % 251) as u8;
    });

    record
}

/// Creates or truncates a file and writes `records` consecutive generated records.
///
/// Returns create, write, or flush errors; errors can leave a partial file.
pub fn generate_input_file(path: impl AsRef<Path>, records: usize) -> io::Result<()> {
    let mut file = File::create(path)?;

    for i in 0..records {
        let record = make_record(i);
        file.write_all(&record)?;
    }

    file.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ScriptedWriter {
        output: Vec<u8>,
        calls: usize,
    }

    impl Write for ScriptedWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.calls += 1;
            match self.calls {
                1 => {
                    self.output.extend_from_slice(&bytes[..2]);
                    Ok(2)
                }
                2 => Err(io::ErrorKind::Interrupted.into()),
                3 => Err(io::ErrorKind::Other.into()),
                _ => {
                    self.output.extend_from_slice(bytes);
                    Ok(bytes.len())
                }
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn retry_after_short_write_and_error_does_not_duplicate_bytes() {
        let mut output = ScriptedWriter {
            output: Vec::new(),
            calls: 0,
        };
        let mut pending = b"abcdef".to_vec();
        assert!(write_pending(&mut output, &mut pending).is_err());
        assert_eq!(output.output, b"ab");
        assert_eq!(pending, b"cdef");
        write_pending(&mut output, &mut pending).unwrap();
        assert_eq!(output.output, b"abcdef");
        assert!(pending.is_empty());
    }

    #[test]
    fn flush_and_close_report_errors_and_drop_does_not_panic() -> io::Result<()> {
        // A read-only real file deterministically rejects writes on every host.
        let mut writer = MyBufWriter {
            file: File::open(std::env::current_exe()?)?,
            buffer: b"pending".to_vec(),
        };
        writer
            .flush()
            .expect_err("read-only file must reject writes");
        assert_eq!(writer.buffer, b"pending");
        writer
            .close()
            .expect_err("close must expose the flush failure");
        // close consumes the writer, so its Drop retry has also completed here.
        Ok(())
    }

    #[test]
    fn zero_progress_write_keeps_pending_bytes() {
        struct ZeroWriter;
        impl Write for ZeroWriter {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Ok(0)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut pending = b"abc".to_vec();
        assert_eq!(
            write_pending(&mut ZeroWriter, &mut pending)
                .unwrap_err()
                .kind(),
            io::ErrorKind::WriteZero
        );
        assert_eq!(pending, b"abc");
    }
}
