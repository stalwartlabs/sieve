use std::slice::Iter;

use mail_parser::{Message, MessagePart, MimeHeaders, PartType};

use crate::Context;

#[derive(Debug)]
pub(crate) enum ContentTypeFilter {
    Type(String),
    TypeSubtype((String, String)),
}

pub(crate) struct SubpartIterator<'x> {
    ctx: &'x Context<'x>,
    iter: Iter<'x, usize>,
    iter_stack: Vec<Iter<'x, usize>>,
    anychild: bool,
}

impl<'x> SubpartIterator<'x> {
    pub(crate) fn new(ctx: &'x Context<'x>, parts: &'x [usize], anychild: bool) -> Self {
        SubpartIterator {
            ctx,
            iter: parts.iter(),
            iter_stack: Vec::new(),
            anychild,
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<(usize, &MessagePart<'x>)> {
        loop {
            if let Some(&part_id) = self.iter.next() {
                let subpart = self.ctx.message.parts.get(part_id)?;
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

impl<'x> Context<'x> {
    pub(crate) fn find_nested_parts<'z: 'x>(
        &'z self,
        mut message: &'x Message<'x>,
        ct_filter: &[ContentTypeFilter],
        visitor_fnc: &mut impl FnMut(&MessagePart, &[u8]) -> bool,
    ) -> bool {
        let mut iter_stack = Vec::new();
        let mut iter = vec![self.part].into_iter();

        loop {
            while let Some(part_id) = iter.next() {
                if let Some(subpart) = message.parts.get(part_id) {
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
                    if process_part && visitor_fnc(subpart, message.raw_message.as_ref()) {
                        return true;
                    }
                    match &subpart.body {
                        PartType::Multipart(subparts) => {
                            iter_stack.push((
                                std::mem::replace(&mut iter, subparts.clone().into_iter()),
                                None,
                            ));
                        }
                        PartType::Message(next_message) => {
                            iter_stack.push((
                                std::mem::replace(&mut iter, vec![0].into_iter()),
                                Some(message),
                            ));
                            message = next_message;
                        }
                        _ => (),
                    }
                }
            }
            if let Some((prev_iter, prev_message)) = iter_stack.pop() {
                iter = prev_iter;
                if let Some(prev_message) = prev_message {
                    message = prev_message;
                }
            } else {
                break;
            }
        }
        false
    }

    pub(crate) fn find_nested_parts_ids(&self, include_current: bool) -> Vec<usize> {
        if self.part == 0 {
            if include_current {
                (0..self.message.parts.len()).collect()
            } else if self.message.parts.len() > 1 {
                (1..self.message.parts.len()).collect()
            } else {
                Vec::new()
            }
        } else {
            let mut part_ids = Vec::new();
            let mut iter_stack = Vec::new();

            if include_current {
                part_ids.push(self.part);
            }

            if let Some(PartType::Multipart(subparts)) =
                self.message.parts.get(self.part).map(|p| &p.body)
            {
                let mut iter = subparts.iter();
                loop {
                    while let Some(&part_id) = iter.next() {
                        part_ids.push(part_id);
                        if let Some(PartType::Multipart(subparts)) =
                            self.message.parts.get(part_id).map(|p| &p.body)
                        {
                            iter_stack.push(std::mem::replace(&mut iter, subparts.iter()));
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
