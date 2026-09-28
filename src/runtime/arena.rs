/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use bumpalo::Bump;
use mail_parser::Message;
use std::{
    cell::{Cell, RefCell},
    ptr::NonNull,
};

#[derive(Default)]
pub struct Arena {
    pub(crate) bump: Bump,
    texts: RefCell<Vec<String>>,
    text_bytes: Cell<usize>,
    messages: Vec<NonNull<Message<'static>>>,
    message_bytes: usize,
    charged: Cell<usize>,
    limit: Option<usize>,
}

unsafe impl Send for Arena {}

impl Arena {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(bytes: usize) -> Self {
        Arena {
            bump: Bump::with_capacity(bytes),
            texts: RefCell::default(),
            text_bytes: Cell::new(0),
            messages: Vec::new(),
            message_bytes: 0,
            charged: Cell::new(0),
            limit: None,
        }
    }

    pub fn reset(&mut self) {
        self.bump.reset();
        self.release();
    }

    pub(crate) fn prepare(&mut self, limit: usize) {
        self.reset();
        if self.bump.allocated_bytes() > limit {
            self.bump = Bump::new();
        }
        self.bump.set_allocation_limit(Some(limit));
        self.limit = Some(limit);
    }

    pub fn allocated_bytes(&self) -> usize {
        self.bump.allocated_bytes() + self.message_bytes + self.text_bytes.get()
    }

    pub(crate) fn charge(&self, bytes: usize) -> bool {
        let charged = self.charged.get().saturating_add(bytes);
        if let Some(limit) = self.limit {
            if self.bump.allocated_bytes().saturating_add(charged) > limit {
                return false;
            }
            self.bump.set_allocation_limit(Some(limit - charged));
        }
        self.charged.set(charged);
        true
    }

    pub(crate) fn keep_text<'x>(&self, text: String) -> &'x str {
        self.text_bytes.set(self.text_bytes.get() + text.len());
        let mut texts = self.texts.borrow_mut();
        texts.push(text);
        let text = texts.last().map_or("", String::as_str);
        unsafe { &*(text as *const str) }
    }

    pub(crate) fn keep_charged_text<'x>(&self, text: String) -> Option<&'x str> {
        self.charge(text.len()).then(|| self.keep_text(text))
    }

    pub(crate) fn keep_message<'x>(&mut self, message: Message<'static>) -> &'x Message<'x> {
        self.message_bytes += message.raw().len();
        let message = NonNull::from(Box::leak(Box::new(message)));
        self.messages.push(message);
        unsafe { message.as_ref() }
    }

    fn release(&mut self) {
        self.texts.get_mut().clear();
        self.text_bytes.set(0);
        self.charged.set(0);
        self.message_bytes = 0;
        for message in self.messages.drain(..) {
            drop(unsafe { Box::from_raw(message.as_ptr()) });
        }
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        self.release();
    }
}
