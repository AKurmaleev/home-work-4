use std::{fs, io, time::Instant};

use hw::{RECORD_SIZE, copy_fast, copy_slow, generate_input_file};

const BENCH_RECORD_COUNT: usize = 100_000;

fn measure(name: &str, f: impl FnOnce() -> io::Result<u64>) -> io::Result<u64> {
    let started = Instant::now();

    let bytes = f()?;

    let elapsed = started.elapsed();
    let mb = bytes as f64 / 1024.0 / 1024.0;
    let seconds = elapsed.as_secs_f64();
    let throughput = mb / seconds;

    println!("{name}: {:.2?}, {:.2} MiB/s", elapsed, throughput);

    Ok(bytes)
}

fn main() -> io::Result<()> {
    let input_path = "input.bin";
    let slow_output_path = "slow.bin";
    let fast_output_path = "fast.bin";

    let total_bytes = RECORD_SIZE * BENCH_RECORD_COUNT;

    println!(
        "Generating input file: {:.2} MiB",
        total_bytes as f64 / 1024.0 / 1024.0
    );

    generate_input_file(input_path, BENCH_RECORD_COUNT)?;

    println!("Running benchmark...");

    let slow_bytes = measure("copy_slow", || copy_slow(input_path, slow_output_path))?;

    let fast_bytes = measure("copy_fast", || copy_fast(input_path, fast_output_path))?;

    assert_eq!(
        slow_bytes, fast_bytes,
        "copy_slow and copy_fast copied different number of bytes"
    );

    let slow_data = fs::read(slow_output_path)?;
    let fast_data = fs::read(fast_output_path)?;

    assert_eq!(
        slow_data, fast_data,
        "slow.bin and fast.bin have different content"
    );

    println!("OK: output files are equal");

    Ok(())
}
