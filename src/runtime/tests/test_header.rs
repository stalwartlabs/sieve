use mail_parser::{parsers::header::parse_header_name, Header, HeaderName};

use crate::{compiler::lexer::string::StringItem, Context};

impl<'x, 'y> Context<'x, 'y> {
    fn find_headers(
        &self,
        header_names: &[StringItem],
        index: Option<i32>,
        any_child: bool,
        mut visitor_fnc: impl FnMut(&Header) -> bool,
    ) -> bool {
        let message = if let Some(message) = &self.message {
            message
        } else {
            #[cfg(test)]
            panic!("Message not set.");
            #[cfg(not(test))]
            return false;
        };

        let mut subparts = if any_child {
            message.get_subparts_recursive(self.part)
        } else {
            None
        };
        let mut message_part = message.parts.get(self.part);
        let header_names = self.eval_strings(header_names);

        while let Some(message_part) = message_part
            .take()
            .or_else(|| subparts.as_mut().and_then(|sp| sp.next()))
        {
            for header_name in &header_names {
                let header_name = HeaderName::from(parse_header_name(header_name.as_bytes()).1);

                match index {
                    None => {
                        for header in message_part
                            .headers
                            .iter()
                            .filter(|h| h.name == header_name)
                        {
                            if visitor_fnc(header) {
                                return true;
                            }
                        }
                    }
                    Some(index) if index >= 0 => {
                        let mut header_count = 0;
                        for header in &message_part.headers {
                            if header.name == header_name {
                                header_count += 1;
                                if header_count == index {
                                    if visitor_fnc(header) {
                                        return true;
                                    }
                                    break;
                                }
                            }
                        }
                    }
                    Some(index) => {
                        let index = -index;
                        let mut header_count = 0;
                        for header in message_part.headers.iter().rev() {
                            if header.name == header_name {
                                header_count += 1;
                                if header_count == index {
                                    if visitor_fnc(header) {
                                        return true;
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }
}
