//! File-I/O contract, buffering-boundary, and record-generation regression tests.

use std::{fs, io};

use hw::{
    BUFFER_SIZE, MyBufReader, MyBufWriter, RECORD_SIZE, copy_fast, copy_slow, generate_input_file,
    make_record,
};

#[test]
fn my_buf_reader_reads_all_bytes() -> io::Result<()> {
    let input = temp_file_path("reader_reads_all_bytes_input.bin");

    fs::write(&input, b"hello buffered reader")?;

    let mut reader = MyBufReader::open(&input)?;

    let mut result = Vec::new();

    while let Some(byte) = reader.read_byte()? {
        result.push(byte);
    }

    assert_eq!(result, b"hello buffered reader");

    drop(reader);
    fs::remove_file(input)?;

    Ok(())
}

#[test]
fn my_buf_reader_handles_empty_file() -> io::Result<()> {
    let input = temp_file_path("reader_handles_empty_file_input.bin");

    fs::write(&input, b"")?;

    let mut reader = MyBufReader::open(&input)?;

    assert_eq!(reader.read_byte()?, None);

    drop(reader);
    fs::remove_file(input)?;

    Ok(())
}

#[test]
fn my_buf_reader_reads_more_than_one_buffer() -> io::Result<()> {
    let input = temp_file_path("reader_reads_more_than_one_buffer_input.bin");

    let data = make_test_data(200_000);
    fs::write(&input, &data)?;

    let mut reader = MyBufReader::open(&input)?;

    let mut result = Vec::new();

    while let Some(byte) = reader.read_byte()? {
        result.push(byte);
    }

    assert_eq!(result, data);

    drop(reader);
    fs::remove_file(input)?;

    Ok(())
}

#[test]
fn my_buf_writer_writes_all_bytes_after_close() -> io::Result<()> {
    let output = temp_file_path("writer_writes_all_bytes_output.bin");

    {
        let mut writer = MyBufWriter::create(&output)?;

        writer.write_buffered(b"hello")?;
        writer.write_buffered(b" ")?;
        writer.write_buffered(b"world")?;

        writer.close()?;
    }

    let result = fs::read(&output)?;

    assert_eq!(result, b"hello world");

    fs::remove_file(output)?;

    Ok(())
}

#[test]
fn my_buf_writer_handles_many_small_writes() -> io::Result<()> {
    let output = temp_file_path("writer_handles_many_small_writes_output.bin");

    {
        let mut writer = MyBufWriter::create(&output)?;

        for _ in 0..10_000 {
            writer.write_buffered(b"ab")?;
        }

        writer.close()?;
    }

    let result = fs::read(&output)?;

    assert_eq!(result.len(), 20_000);

    for chunk in result.chunks(2) {
        assert_eq!(chunk, b"ab");
    }

    fs::remove_file(output)?;

    Ok(())
}

#[test]
fn my_buf_writer_flush_writes_data_without_close() -> io::Result<()> {
    let output = temp_file_path("writer_flush_writes_data_output.bin");

    {
        let mut writer = MyBufWriter::create(&output)?;

        writer.write_buffered(b"abc")?;
        writer.flush()?;

        let result = fs::read(&output)?;
        assert_eq!(result, b"abc");

        writer.write_buffered(b"def")?;
        writer.close()?;
    }

    let result = fs::read(&output)?;

    assert_eq!(result, b"abcdef");

    fs::remove_file(output)?;

    Ok(())
}

#[test]
fn copy_fast_produces_same_output_as_copy_slow() -> io::Result<()> {
    let input = temp_file_path("copy_input.bin");
    let slow = temp_file_path("copy_slow_output.bin");
    let fast = temp_file_path("copy_fast_output.bin");

    generate_input_file(&input, 10_000)?;

    let slow_bytes = copy_slow(&input, &slow)?;
    let fast_bytes = copy_fast(&input, &fast)?;

    assert_eq!(slow_bytes, fast_bytes);

    let slow_data = fs::read(&slow)?;
    let fast_data = fs::read(&fast)?;

    assert_eq!(slow_data, fast_data);

    fs::remove_file(input)?;
    fs::remove_file(slow)?;
    fs::remove_file(fast)?;

    Ok(())
}

#[test]
fn copy_fast_handles_empty_file() -> io::Result<()> {
    let input = temp_file_path("copy_empty_input.bin");
    let output = temp_file_path("copy_empty_output.bin");

    fs::write(&input, b"")?;

    let copied = copy_fast(&input, &output)?;

    assert_eq!(copied, 0);
    assert_eq!(fs::read(&output)?, b"");

    fs::remove_file(input)?;
    fs::remove_file(output)?;

    Ok(())
}

#[test]
fn copy_fast_handles_small_file() -> io::Result<()> {
    let input = temp_file_path("copy_small_input.bin");
    let output = temp_file_path("copy_small_output.bin");

    fs::write(&input, b"small file")?;

    let copied = copy_fast(&input, &output)?;

    assert_eq!(copied, 10);
    assert_eq!(fs::read(&output)?, b"small file");

    fs::remove_file(input)?;
    fs::remove_file(output)?;

    Ok(())
}

#[test]
fn copy_fast_handles_large_file() -> io::Result<()> {
    let input = temp_file_path("copy_large_input.bin");
    let output = temp_file_path("copy_large_output.bin");

    let data = make_test_data(500_000);
    fs::write(&input, &data)?;

    let copied = copy_fast(&input, &output)?;

    assert_eq!(copied, data.len() as u64);
    assert_eq!(fs::read(&output)?, data);

    fs::remove_file(input)?;
    fs::remove_file(output)?;

    Ok(())
}

#[test]
fn reader_handles_exact_buffer_boundaries_and_binary_bytes() -> io::Result<()> {
    for (case, length) in [
        BUFFER_SIZE - 1,
        BUFFER_SIZE,
        BUFFER_SIZE + 1,
        2 * BUFFER_SIZE + 3,
    ]
    .into_iter()
    .enumerate()
    {
        let input = temp_file_path(&format!("reader_boundary_{case}.bin"));
        let data = make_test_data(length);
        fs::write(&input, &data)?;
        let mut reader = MyBufReader::open(&input)?;
        let mut actual = Vec::new();
        while let Some(byte) = reader.read_byte()? {
            actual.push(byte);
        }
        assert_eq!(actual, data);
        assert_eq!(reader.read_byte()?, None);
        drop(reader);
        fs::remove_file(input)?;
    }
    Ok(())
}

#[test]
fn writer_handles_boundaries_empty_writes_and_repeated_flush() -> io::Result<()> {
    for (case, length) in [
        0,
        BUFFER_SIZE - 1,
        BUFFER_SIZE,
        BUFFER_SIZE + 1,
        2 * BUFFER_SIZE + 3,
    ]
    .into_iter()
    .enumerate()
    {
        let output = temp_file_path(&format!("writer_boundary_{case}.bin"));
        let data = make_test_data(length);
        let mut writer = MyBufWriter::create(&output)?;
        writer.write_buffered(&[])?;
        writer.write_buffered(&data)?;
        assert_eq!(
            fs::metadata(&output)?.len(),
            ((length / BUFFER_SIZE) * BUFFER_SIZE) as u64
        );
        writer.flush()?;
        writer.flush()?;
        writer.close()?;
        assert_eq!(fs::read(&output)?, data);
        fs::remove_file(output)?;
    }
    Ok(())
}

#[test]
fn records_preserve_modulo_pattern_at_usize_boundaries() {
    for index in [
        0,
        1,
        250,
        251,
        usize::MAX - RECORD_SIZE,
        usize::MAX - 1,
        usize::MAX,
    ] {
        let expected = std::array::from_fn(|offset| {
            // Widen before addition so the oracle does not repeat usize overflow.
            u8::try_from((index as u128 + offset as u128) % 251).unwrap()
        });
        assert_eq!(make_record(index), expected, "index={index}");
    }
}

#[test]
fn dropping_writer_flushes_pending_bytes() -> io::Result<()> {
    let output = temp_file_path("writer_drop_output.bin");
    let mut writer = MyBufWriter::create(&output)?;
    writer.write_buffered(b"pending")?;
    drop(writer);
    assert_eq!(fs::read(&output)?, b"pending");
    fs::remove_file(output)?;
    Ok(())
}

fn temp_file_path(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();

    let unique_name = format!("l7_homework_{}_{}", std::process::id(), name);

    path.push(unique_name);
    path
}

fn make_test_data(len: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(len);

    for i in 0..len {
        data.push((i % 251) as u8);
    }

    data
}
