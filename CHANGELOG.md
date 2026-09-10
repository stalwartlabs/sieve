sieve-rs 1.0.0
================================
- Scripts compile to a flat bytecode; `Sieve::from_bytes` is a zero-copy borrow of the stored bytes instead of a full deserialization.
- The event loop was replaced by the `Handler` trait: synchronous callbacks with `Reply::Pending` to suspend for asynchronous work, resumed with `Context::resume`.
- Scripts are passed by reference (`&Sieve`), included scripts live in the caller or in `Runtime::with_include_script`.
- `Context::new`, `Runtime::filter` and `Runtime::filter_parsed` take a caller-owned `Arena`.
- Regular expressions compile lazily per loaded script; glob patterns are precompiled into the bytecode.
- Removed the `rkyv` feature and the `arc-swap` dependency; added `bumpalo`, `memchr` and `smallvec`.

sieve-rs 0.8.1
================================
- Reduce enum variants with `Box`.
- Use of `hashify` everywhere.

sieve-rs 0.8.0
================================
- Serialization format improvements.

sieve-rs 0.7.3
================================
- Bump `mail-builder` dependency to 0.5.

sieve-rs 0.7.2
================================
- Fix: `replace` action adds additional `From` header.

sieve-rs 0.7.1
================================
- Bump `fancy-regex` dependency to 0.17.0.
- Fixed `rkyv` serialization lifetimes for rustc 1.93.0.

sieve-rs 0.7.0
================================
- Added `rkyv` support.
- Bump `mail-parser` dependency to 0.11.0.
- Fix: Allow redirect to sender (#11).

sieve-rs 0.6.0
================================
- Replaced `phf` with `hashify`.
- Bump `mail-parser` dependency to 0.10.0.

sieve-rs 0.5.3
================================
- Fixed `register_match_var` function.

sieve-rs 0.5.2
================================
- Fixed `set_global_variable` function.

sieve-rs 0.5.1
================================
- Case insensitive envelope tests (#6).
- Set envelope variables internally (#10).

sieve-rs 0.5.0
================================
- Removed context.

sieve-rs 0.4.0
================================
- Support for expressions.

sieve-rs 0.3.1
================================
- Bump `mail-builder` dependency to 0.3.0.

sieve-rs 0.3.0
================================
- Updated ``execute`` grammar.
- Upgraded to latest mail-parser.
- Envelope accessible from environment variables.

sieve-rs 0.2.0
================================
- Improved event loop.

sieve-rs 0.1.0
================================
- Initial release.
