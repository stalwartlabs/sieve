use std::slice::Iter;

use mail_parser::{Message, MessageAttachment, MessagePart, MimeHeaders, PartType};

use crate::Context;

use super::test_body::MAX_NEST_LEVELS;

#[derive(Debug)]
pub(crate) enum ContentTypeFilter {
    Type(String),
    TypeSubtype((String, String)),
}

pub(crate) struct SubpartIterator<'x> {
    message: &'x Message<'x>,
    ctx: &'x Context<'x>,
    iter: Iter<'x, usize>,
    iter_stack: Vec<Iter<'x, usize>>,
    anychild: bool,
}

impl<'x> SubpartIterator<'x> {
    pub(crate) fn new(
        ctx: &'x Context<'x>,
        message: &'x Message<'x>,
        parts: &'x [usize],
        anychild: bool,
    ) -> Self {
        SubpartIterator {
            ctx,
            message,
            iter: parts.iter(),
            iter_stack: Vec::new(),
            anychild,
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<(usize, &MessagePart<'x>)> {
        loop {
            if let Some(&part_id) = self.iter.next() {
                if self.ctx.part_deletions.contains(&part_id) {
                    if let Some(replaced_part) = self
                        .ctx
                        .part_replacements
                        .iter()
                        .find(|p| p.part_id == part_id)
                    {
                        return Some((part_id, &replaced_part.part));
                    } else {
                        continue;
                    }
                }

                let subpart = self.message.parts.get(part_id)?;
                match &subpart.body {
                    PartType::Multipart(subparts) if self.anychild => {
                        self.iter_stack
                            .push(std::mem::replace(&mut self.iter, subparts.iter()));
                    }
                    _ => (),
                }
                return Some((part_id, subpart));
            }
            if let Some(prev_iter) = self.iter_stack.pop() {
                self.iter = prev_iter;
            } else {
                return None;
            }
        }
    }
}

pub(crate) trait NestedParts {
    fn find_nested_parts(
        &self,
        ctx: &Context,
        ct_filter: &[ContentTypeFilter],
        max_nest_levels: usize,
        visitor_fnc: &mut impl FnMut(&MessagePart, &[u8]) -> bool,
    ) -> bool;

    fn find_nested_parts_ids(&self, ctx: &Context, include_current: bool) -> Vec<usize>;
}

impl<'x> NestedParts for Message<'x> {
    fn find_nested_parts(
        &self,
        ctx: &Context,
        ct_filter: &[ContentTypeFilter],
        max_nest_levels: usize,
        visitor_fnc: &mut impl FnMut(&MessagePart, &[u8]) -> bool,
    ) -> bool {
        let mut message = self;
        let raw_bytes = message.raw_message.as_ref();
        let mut iter_stack = Vec::new();
        let mut iter = vec![if max_nest_levels == MAX_NEST_LEVELS {
            ctx.part
        } else {
            0
        }]
        .into_iter();
        let mut nest_levels = MAX_NEST_LEVELS - max_nest_levels;

        loop {
            while let Some(part_id) = iter.next() {
                let subpart = if nest_levels == 0 {
                    ctx.get_part(message, part_id)
                } else {
                    message.parts.get(part_id)
                };

                if let Some(subpart) = subpart {
                    let process_part = if !ct_filter.is_empty() {
                        let mut process_part = false;
                        let (ct, cst) = if let Some(ct) = subpart.get_content_type() {
                            (ct.c_type.as_ref(), ct.c_subtype.as_deref().unwrap_or(""))
                        } else {
                            match &subpart.body {
                                PartType::Text(_) => ("text", "plain"),
                                PartType::Html(_) => ("text", "html"),
                                PartType::Message(_) => ("message", "rfc822"),
                                PartType::Multipart(_) => ("multipart", "mixed"),
                                _ => ("application", "octet-stream"),
                            }
                        };

                        for ctf in ct_filter {
                            match ctf {
                                ContentTypeFilter::Type(name) => {
                                    if name.eq_ignore_ascii_case(ct) {
                                        process_part = true;
                                        break;
                                    }
                                }
                                ContentTypeFilter::TypeSubtype((name, subname)) => {
                                    if name.eq_ignore_ascii_case(ct)
                                        && subname.eq_ignore_ascii_case(cst)
                                    {
                                        process_part = true;
                                        break;
                                    }
                                }
                            }
                        }

                        process_part
                    } else {
                        true
                    };
                    if process_part && visitor_fnc(subpart, raw_bytes) {
                        return true;
                    }
                    match &subpart.body {
                        PartType::Multipart(subparts) => {
                            iter_stack.push((
                                std::mem::replace(&mut iter, subparts.clone().into_iter()),
                                None,
                            ));
                        }
                        PartType::Message(MessageAttachment::Parsed(next_message)) => {
                            iter_stack.push((
                                std::mem::replace(&mut iter, vec![0].into_iter()),
                                Some(message),
                            ));
                            message = next_message.as_ref();
                            nest_levels += 1;
                        }
                        PartType::Message(MessageAttachment::Raw(raw_message))
                            if max_nest_levels > 0 =>
                        {
                            if let Some(message) = Message::parse(raw_message) {
                                if message.find_nested_parts(
                                    ctx,
                                    ct_filter,
                                    max_nest_levels - 1,
                                    visitor_fnc,
                                ) {
                                    return true;
                                }
                            }
                        }
                        _ => (),
                    }
                }
            }
            if let Some((prev_iter, prev_message)) = iter_stack.pop() {
                iter = prev_iter;
                if let Some(prev_message) = prev_message {
                    message = prev_message;
                    nest_levels -= 1;
                }
            } else {
                break;
            }
        }
        false
    }

    fn find_nested_parts_ids(&self, ctx: &Context, include_current: bool) -> Vec<usize> {
        if ctx.part == 0 {
            if !ctx.part_deletions.is_empty() {
                let mut part_ids = Vec::new();
                if include_current {
                    part_ids.push(0);
                }
                if self.parts.len() > 1 {
                    for part_id in 1..self.parts.len() {
                        if !ctx.part_deletions.contains(&part_id)
                            || ctx.part_replacements.iter().any(|p| p.part_id == part_id)
                        {
                            part_ids.push(part_id);
                        }
                    }
                }
                part_ids
            } else if include_current {
                (0..self.parts.len()).collect()
            } else if self.parts.len() > 1 {
                (1..self.parts.len()).collect()
            } else {
                Vec::new()
            }
        } else {
            let mut part_ids = Vec::new();
            let mut iter_stack = Vec::new();

            if include_current {
                part_ids.push(ctx.part);
            }

            if let Some(PartType::Multipart(subparts)) = self.parts.get(ctx.part).map(|p| &p.body) {
                let mut iter = subparts.iter();
                loop {
                    while let Some(&part_id) = iter.next() {
                        if !ctx.part_deletions.contains(&part_id)
                            || ctx.part_replacements.iter().any(|p| p.part_id == part_id)
                        {
                            part_ids.push(part_id);
                            if let Some(PartType::Multipart(subparts)) =
                                self.parts.get(part_id).map(|p| &p.body)
                            {
                                iter_stack.push(std::mem::replace(&mut iter, subparts.iter()));
                            }
                        }
                    }
                    if let Some(prev_iter) = iter_stack.pop() {
                        iter = prev_iter;
                    } else {
                        break;
                    }
                }
            }

            part_ids
        }
    }
}

impl ContentTypeFilter {
    pub(crate) fn parse(ct: &str) -> Option<ContentTypeFilter> {
        let mut iter = ct.split('/');
        let name = iter.next()?;
        if let Some(sub_name) = iter.next() {
            if !name.is_empty() && !sub_name.is_empty() && iter.next().is_none() {
                Some(ContentTypeFilter::TypeSubtype((
                    name.to_string(),
                    sub_name.to_string(),
                )))
            } else {
                None
            }
        } else if !name.is_empty() {
            Some(ContentTypeFilter::Type(name.to_string()))
        } else {
            None
        }
    }
}
