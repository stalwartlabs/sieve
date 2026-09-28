/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::Context;
use mail_parser::{Message, MessageParser, MessagePart, PartId};

pub(crate) const ROOT_PART: PartId = 0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PartCursor {
    next: PartId,
    end: PartId,
    nested: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    Message,
    Nested,
}

impl PartCursor {
    pub(crate) fn subtree(part: MessagePart<'_>, include_self: bool, scope: Scope) -> Self {
        let nested = scope == Scope::Nested;
        PartCursor {
            next: part.id() + u32::from(!include_self),
            end: if nested || !part.is_message() {
                subtree_end(part)
            } else {
                part.id() + 1
            },
            nested,
        }
    }
}

pub(crate) fn empty_message() -> Message<'static> {
    MessageParser::new()
        .parse_owned(b"\r\n".to_vec())
        .unwrap_or_default()
}

pub(crate) fn subtree_end(part: MessagePart<'_>) -> PartId {
    let first = part.id();
    let mut last = part;
    loop {
        if let Some(child) = last.children().next_back() {
            if child.id() <= last.id() {
                break;
            }
            last = child;
        } else if let Some(nested) = last.nested() {
            let root = nested.root_part();
            if root.id() <= last.id() || root.id() == PartId::MAX {
                break;
            }
            last = root;
        } else {
            break;
        }
    }
    last.id().max(first).saturating_add(1)
}

impl<'x> Context<'x> {
    #[inline(always)]
    pub(crate) fn root_part(&self) -> Option<MessagePart<'x>> {
        self.message.part(ROOT_PART)
    }

    #[inline(always)]
    pub(crate) fn current_part(&self) -> Option<MessagePart<'x>> {
        self.message.part(self.part)
    }

    #[inline]
    pub(crate) fn advance(&self, cursor: &mut PartCursor) -> Option<MessagePart<'x>> {
        while cursor.next < cursor.end {
            let id = cursor.next;
            if let Some(end) = self.edits.hidden_end(id) {
                cursor.next = end;
                continue;
            }
            let Some(part) = self.message.part(id) else {
                cursor.next = cursor.end;
                return None;
            };
            cursor.next = if !cursor.nested && part.is_message() {
                subtree_end(part)
            } else {
                id + 1
            };
            return Some(part);
        }
        None
    }
}
