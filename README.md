# Homework 4 — Buffered file I/O

Assignment based on lecture 8, “The Storage”: implement custom `MyBufReader` and `MyBufWriter` without `std::io::BufReader` or `std::io::BufWriter`. The package is named `hw`; signatures from the original template are preserved.

## Implementation

- `src/lib.rs`: `MyBufReader` returns one byte at a time but reads the file in blocks of up to 64 KiB. `position` and `filled` track the unread portion of the buffer; zero returned by `File::read` means end of file.
- `MyBufWriter` accepts arbitrary chunks of bytes, accumulates up to 64 KiB, and writes a block when the buffer is full or on `flush()` or `close()`. On a short write or error, it retains only the unwritten suffix; interrupted calls are retried.
- The original `copy_slow` and `copy_fast` retain the same byte-by-byte outer loop: the difference comes specifically from buffering inside the reader and writer.
- `tests/io_tests.rs` and the unit tests in `src/lib.rs` cover empty files, binary data, 64 KiB boundaries, multiple blocks, identical copies, and an error after a partial write.

**Limitation:** `flush()` transfers data from the application buffer to the file object but does not guarantee durability after a power failure (`sync_all` is not required here). After a `write_buffered()` error, some input may already have been accepted: the entire same slice must not be retried without accounting for progress.

## Verification

```sh
cargo test
cargo run --release
```

The demonstration in `src/main.rs` creates `input.bin`, `slow.bin`, and `fast.bin` in the current directory, copies approximately 0.95 MiB using both methods, and compares the contents. These files are excluded from Git. The program takes **one** measurement per method; the numbers do not establish a statistically supported speedup. A rigorous comparison requires warm-up, repeated independent runs, and control of the OS cache and system load.

Both copy implementations take `O(n)` time. The fast version additionally uses `O(64 KiB)` for each of its two buffers (`128 KiB` total, excluding OS buffers); the slow version does not allocate these application buffers. This memory trade-off reduces the number of calls to `File` from order `n` to order `n / 65536`.
