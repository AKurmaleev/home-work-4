# Homework 4 — Buffered file I/O

Assignment based on lecture 8, “The Storage”: implement custom `MyBufReader` and `MyBufWriter` without `std::io::BufReader` or `std::io::BufWriter`. The package is named `hw`; signatures from the original template are preserved.

The original assignment and scaffold are available in the workspace's `../hw.zip` (`README.md`, `src/lib.rs`, `src/main.rs`, and `tests/io_tests.rs`). The assignment requires keeping `copy_fast`'s byte-at-a-time loop unchanged. Requires stable Rust 1.85 or newer (edition 2024); no external dependencies.

## Implementation

- `src/lib.rs`: `MyBufReader` returns one byte at a time but reads the file in blocks of up to 64 KiB. `position` and `filled` track the unread portion of the buffer; zero returned by `File::read` means end of file.
- `MyBufWriter` accepts arbitrary chunks of bytes, accumulates up to 64 KiB, and writes a block when the buffer is full or on `flush()` or `close()`. On a short write or error, it retains only the unwritten suffix; interrupted calls are retried.
- The original `copy_slow` and `copy_fast` retain the same byte-by-byte outer loop: the difference comes specifically from buffering inside the reader and writer.
- `tests/io_tests.rs` and the unit tests in `src/lib.rs` cover empty files, binary data, 64 KiB boundaries, multiple blocks, identical copies, an error after a partial write, flush/close failures, destructor fallback, and record generation at `usize` limits.
- `make_record` computes each byte as the mathematical `(index + offset) % 251`, reducing the index before addition so every `usize` index behaves identically in debug and release.

**Limitations:** `flush()` transfers data from the application buffer to the file object but does not guarantee durability after a power failure (`sync_all` is not required here). Call `flush()` or `close()` explicitly for checked completion: destructor fallback cannot report file-write errors and can lose pending data. After a `write_buffered()` error, some input may already have been accepted, but the API does not return that count: abandon the transfer or restart with a fresh destination rather than replaying the same slice on the same writer. Copy input and output must name distinct files; errors can leave partial output.

## Verification

```sh
cargo fmt --all --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings -D unused_must_use -D clippy::let_underscore_must_use -D clippy::unused_result_ok
cargo test --locked
cargo test --locked --release
cargo run --locked --release
```

The demonstration in `src/main.rs` creates `input.bin`, `slow.bin`, and `fast.bin` in the current directory, copies approximately 0.95 MiB using both methods, and compares the contents. These files are excluded from Git. The program takes **one** measurement per method; the numbers do not establish a statistically supported speedup. A rigorous comparison requires warm-up, repeated independent runs, and control of the OS cache and system load.

Both copy implementations take `O(n)` time. The fast version additionally uses `O(64 KiB)` for each of its two buffers (`128 KiB` total, excluding OS buffers); the slow version does not allocate these application buffers. For full-block reads and writes, this memory trade-off reduces calls to `File` from order `n` to approximately `n / 65536` per direction, plus the final partial block and EOF check. Short transfers and interruptions can require additional calls. The demonstration does not control OS file-cache state or synchronize writes to durable storage, so its timings are not measurements of device throughput.
