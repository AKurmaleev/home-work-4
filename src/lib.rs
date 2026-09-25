use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

pub const BUFFER_SIZE: usize = 64 * 1024;

// -----------------------------------------------------------------------------
// MyBufReader
// -----------------------------------------------------------------------------

/// Reads one byte at a time while refilling a 64 KiB application buffer.
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
    /// On error, some bytes may already have been accepted or written; do not replay
    /// the entire input slice without tracking progress.
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
        // Ошибку из Drop вернуть нельзя.
        // Поэтому в реальном коде лучше явно вызывать close() или flush().
        let _ = self.flush();
    }
}

// -----------------------------------------------------------------------------
// Медленная версия
// -----------------------------------------------------------------------------

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
// Быстрая версия
// -----------------------------------------------------------------------------
// copy_fast специально тоже использует побайтный API.
// Разница должна быть не в коде копирования, а в реализации MyBufReader и MyBufWriter
// эту функцию не нужно менять, она должна работать с любыми реализациями MyBufReader и MyBufWriter,
// которые вы сделаете
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

pub const RECORD_SIZE: usize = 10;

pub fn make_record(index: usize) -> [u8; RECORD_SIZE] {
    let mut record = [0u8; RECORD_SIZE];

    (0..RECORD_SIZE).for_each(|i| {
        record[i] = ((index + i) % 251) as u8;
    });

    record
}

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
