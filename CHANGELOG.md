# Change Log

All notable changes to this project will be documented in this file. This project adheres to [Semantic Versioning](https://semver.org/).

## [2.0.0] - 2026-09-XX

### Added
- `Runtime::with_max_header_count` and `Runtime::with_max_header_block_size` (and their `set_` forms) limit the number of fields and the size in bytes of the root message's top-level header section in the edited message.

### Changed
- Bump to `mail-parser` 1.0, whose parsed messages are read-only.
- `Context::new` and `Runtime::filter_parsed` borrow the parsed message (`&'x Message<'x>`) instead of taking ownership, and `Context::message` returns that reference, without the edits made by the script so far. Header values and body text are borrowed from the message rather than copied into the arena.
- `addheader`, `deleteheader`, `replace` and `convert` no longer modify the message: the context records the edits and applies them when it builds the new message. `enclose` builds the enclosed message and parses it again; the arena owns it until the next run.
- `Runtime::filter` parses a copy of the raw message, owned by the arena.
- `Context::received_part` takes the `Received` view by value.
- The default protected headers are `HeaderName::OriginalSubject` and `HeaderName::OriginalFrom`, which mail-parser 1.0 parses as known names.
- Decoded body text is cached per part for the duration of a run, so repeated `body` tests decode each part once.
- Text derived from the bodies set by `convert` and `replace`, and the messages built by `enclose`, count against the memory limit. The copy of the raw message made by `Runtime::filter` and body text decoded from the message do not. `Arena::allocated_bytes` includes the messages and texts the arena keeps.
- `foreverypart`, `:anychild` and the `body` test walk the parts with a cursor over part ids instead of allocating a list of ids.
- `convert` from `text/plain` to `text/html` escapes `&` and `>` (mail-parser 1.0 `text_to_html`).
- Bump to `hashify` 0.3.

### Removed
- `Context::take_message`: the caller owns the parsed message.

### Fixed
- `replace` on a multipart part inside `foreverypart` no longer shifts the ids of the parts that follow it.
- `replace` on the root part inside `foreverypart` no longer panics.
- `body :content "message/rfc822"` on an attached message sent with a base64 or quoted-printable transfer encoding reads the decoded message.
- Headers added with `addheader` are parsed like the message's own headers by the `address` and `date` tests and by header variables.
- Headers written by `replace` and `enclose` (From, Subject, Date, Message-ID, Content-Type) are matched by name in later tests.

## [1.0.2] - 2026-09-22

### Fixed
- Line number error fixes.

## [1.0.1] - 2026-09-12

### Changed
- Bump to `mail-builder` 1.0.0.

## [1.0.0] - 2026-09-10

### Changed
- Scripts compile to a flat bytecode; `Sieve::from_bytes` is a zero-copy borrow of the stored bytes instead of a full deserialization.
- The event loop was replaced by the `Handler` trait: synchronous callbacks with `Reply::Pending` to suspend for asynchronous work, resumed with `Context::resume`.
- Scripts are passed by reference (`&Sieve`), included scripts live in the caller or in `Runtime::with_include_script`.
- `SieveAction::SendMessage` carries a `MessageSource` (`Redirect`, `Vacation` or `Notification`).
- `Context::new`, `Runtime::filter` and `Runtime::filter_parsed` take a caller-owned `Arena`.
- Regular expressions compile lazily per loaded script; glob patterns are precompiled into the bytecode.
- Added `bumpalo`, `memchr` and `smallvec` dependencies.

### Removed
- The `rkyv` feature and the `arc-swap` dependency.

## [0.8.1] - 2026-08-22

### Changed
- Reduce enum variants with `Box`.
- Use of `hashify` everywhere.

## [0.8.0] - 2026-08-21

### Changed
- Serialization format improvements.

## [0.7.3] - 2026-08-18

### Changed
- Bump `mail-builder` dependency to 0.5.

## [0.7.2] - 2026-05-19

### Fixed
- `replace` action adds additional `From` header.

## [0.7.1] - 2026-01-29

### Changed
- Bump `fancy-regex` dependency to 0.17.0.

### Fixed
- `rkyv` serialization lifetimes for rustc 1.93.0.

## [0.7.0] - 2025-05-11

### Added
- `rkyv` support.

### Changed
- Bump `mail-parser` dependency to 0.11.0.

### Fixed
- Allow redirect to sender (#11).

## [0.6.0] - 2025-01-26

### Changed
- Replaced `phf` with `hashify`.
- Bump `mail-parser` dependency to 0.10.0.

## [0.5.3] - 2024-11-22

### Fixed
- `register_match_var` function.

## [0.5.2] - 2024-10-05

### Fixed
- `set_global_variable` function.

## [0.5.1] - 2024-09-02

### Changed
- Set envelope variables internally (#10).

### Fixed
- Case insensitive envelope tests (#6).

## [0.5.0] - 2024-03-28

### Removed
- Context.

## [0.4.0] - 2023-12-28

### Added
- Support for expressions.

## [0.3.1] - 2023-06-02

### Changed
- Bump `mail-builder` dependency to 0.3.0.

## [0.3.0] - 2023-02-08

### Added
- Envelope accessible from environment variables.

### Changed
- Updated `execute` grammar.
- Upgraded to latest `mail-parser`.

## [0.2.0] - 2022-10-28

### Changed
- Improved event loop.

## [0.1.0] - 2022-10-21

### Added
- Initial release.
