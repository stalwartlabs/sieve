/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
mod native {
    use mail_builder::headers::message_id::generate_message_id_header;
    #[cfg(not(test))]
    use mail_builder::mime;
    use std::time::SystemTime;

    #[inline(always)]
    pub(crate) fn unix_time() -> i64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0) as i64
    }

    #[inline(always)]
    #[cfg(not(test))]
    pub(crate) fn make_boundary() -> String {
        mime::make_boundary(".")
    }

    #[inline(always)]
    pub(crate) fn write_message_id(output: &mut Vec<u8>, hostname: &str) {
        generate_message_id_header(output, hostname);
    }
}

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
mod wasm {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0x5deece66d);

    fn next_token() -> (u64, u64) {
        let counter = COUNTER.fetch_add(0x9e3779b97f4a7c15, Ordering::Relaxed);
        let mut mixed = counter;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d049bb133111eb);
        (mixed ^ (mixed >> 31), counter)
    }

    pub(crate) fn unix_time() -> i64 {
        0
    }

    pub(crate) fn make_boundary() -> String {
        let (mixed, counter) = next_token();
        format!("{mixed:016x}.{:08x}", counter as u32)
    }

    pub(crate) fn write_message_id(output: &mut Vec<u8>, hostname: &str) {
        output.push(b'<');
        output.extend_from_slice(make_boundary().as_bytes());
        output.push(b'@');
        output.extend_from_slice(hostname.as_bytes());
        output.push(b'>');
    }
}

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
pub(crate) use native::*;
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub(crate) use wasm::*;
