use std::slice::Iter;

use mail_parser::{Message, MessageAttachment, MessagePart, MimeHeaders, PartType};

pub(crate) enum ContentTypeFilter {
    Type(String),
    TypeSubtype((String, String)),
}

pub(crate) struct SubpartIterator<'x> {
    message: &'x Message<'x>,
    iter: Iter<'x, usize>,
    iter_stack: Vec<Iter<'x, usize>>,
    anychild: bool,
}

impl<'x> SubpartIterator<'x> {
    pub(crate) fn new(message: &'x Message<'x>, parts: &'x [usize], anychild: bool) -> Self {
        SubpartIterator {
            message,
            iter: parts.iter(),
            iter_stack: Vec::new(),
            anychild,
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&MessagePart<'x>> {
        loop {
            if let Some(part_id) = self.iter.next() {
                let subpart = self.message.parts.get(*part_id)?;
                match &subpart.body {
                    PartType::Multipart(subparts) if self.anychild => {
                        self.iter_stack
                            .push(std::mem::replace(&mut self.iter, subparts.iter()));
                    }
                    _ => (),
                }
                return Some(subpart);
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
        part_id: usize,
        ct_filter: &[ContentTypeFilter],
        max_nest_levels: usize,
        visitor_fnc: &mut impl FnMut(&MessagePart, &[u8]) -> bool,
    ) -> bool;
}

impl<'x> NestedParts for Message<'x> {
    fn find_nested_parts(
        &self,
        part_id: usize,
        ct_filter: &[ContentTypeFilter],
        max_nest_levels: usize,
        visitor_fnc: &mut impl FnMut(&MessagePart, &[u8]) -> bool,
    ) -> bool {
        let mut message = self;
        let raw_bytes = message.raw_message.as_ref();
        let mut iter_stack = Vec::new();
        let mut iter = vec![part_id].into_iter();

        loop {
            while let Some(part_id) = iter.next() {
                if let Some(subpart) = message.parts.get(part_id) {
                    let process_part = if !ct_filter.is_empty() {
                        let mut process_part = false;
                        if let Some(ct) = subpart.get_content_type() {
                            for ctf in ct_filter {
                                match ctf {
                                    ContentTypeFilter::Type(name) => {
                                        if name.eq_ignore_ascii_case(ct.c_type.as_ref()) {
                                            process_part = true;
                                            break;
                                        }
                                    }
                                    ContentTypeFilter::TypeSubtype((name, subname)) => {
                                        if let Some(subtype) = &ct.c_subtype {
                                            if name.eq_ignore_ascii_case(ct.c_type.as_ref())
                                                && subname.eq_ignore_ascii_case(subtype.as_ref())
                                            {
                                                process_part = true;
                                                break;
                                            }
                                        }
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
                        }
                        PartType::Message(MessageAttachment::Raw(raw_message))
                            if max_nest_levels > 0 =>
                        {
                            if let Some(message) = Message::parse(raw_message) {
                                if message.find_nested_parts(
                                    0,
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
                } else {
                    #[cfg(test)]
                    panic!("Part id {} does not exist.", part_id);
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
