/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::{
    Context,
    runtime::message::parts::{PartCursor, Scope},
};
use mail_parser::{MessagePart, PartKind};

#[derive(Debug)]
pub(crate) enum ContentTypeFilter<'x> {
    Type(&'x str),
    TypeSubtype((&'x str, &'x str)),
}

impl<'x> Context<'x> {
    pub(crate) fn find_nested_parts(
        &self,
        ct_filter: &[ContentTypeFilter<'_>],
        visitor_fnc: &mut impl FnMut(MessagePart<'x>) -> bool,
    ) -> bool {
        let Some(part) = self.current_part() else {
            return false;
        };
        let mut parts = PartCursor::subtree(part, true, Scope::Nested);
        while let Some(part) = self.advance(&mut parts) {
            if (ct_filter.is_empty() || self.matches_content_type(part, ct_filter))
                && visitor_fnc(part)
            {
                return true;
            }
        }
        false
    }

    fn matches_content_type(
        &self,
        part: MessagePart<'x>,
        ct_filter: &[ContentTypeFilter<'_>],
    ) -> bool {
        let (ct, cst) =
            self.part_content_type(part)
                .unwrap_or_else(|| match self.part_kind(part) {
                    PartKind::Text => ("text", "plain"),
                    PartKind::Html => ("text", "html"),
                    PartKind::Message(_) => ("message", "rfc822"),
                    PartKind::Multipart => ("multipart", "mixed"),
                    _ => ("application", "octet-stream"),
                });

        ct_filter.iter().any(|ctf| match ctf {
            ContentTypeFilter::Type(name) => name.eq_ignore_ascii_case(ct),
            ContentTypeFilter::TypeSubtype((name, subname)) => {
                name.eq_ignore_ascii_case(ct) && subname.eq_ignore_ascii_case(cst)
            }
        })
    }
}

impl<'x> ContentTypeFilter<'x> {
    pub(crate) fn parse(ct: &'x str) -> Option<ContentTypeFilter<'x>> {
        let mut iter = ct.split('/');
        let name = iter.next()?;
        if let Some(sub_name) = iter.next() {
            if !name.is_empty() && !sub_name.is_empty() && iter.next().is_none() {
                Some(ContentTypeFilter::TypeSubtype((name, sub_name)))
            } else {
                None
            }
        } else if !name.is_empty() {
            Some(ContentTypeFilter::Type(name))
        } else {
            None
        }
    }
}
