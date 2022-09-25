use super::{Token, WORDS};

#[derive(Debug)]
pub(crate) struct TokenInfo {
    pub(crate) token: Token,
    pub(crate) line_num: usize,
    pub(crate) line_pos: usize,
}

enum State {
    None,
    BracketComment { line_num: usize, line_pos: usize },
    HashComment,
    QuotedString { line_num: usize, line_pos: usize },
    MultiLine { line_num: usize, line_pos: usize },
}

#[derive(Debug)]
pub struct Error {
    line_num: usize,
    line_pos: usize,
    error_type: ErrorType,
}

#[derive(Debug)]
pub enum ErrorType {
    InvalidCharacter(u8),
    UnterminatedString,
    UnterminatedComment,
    UnterminatedMultiline,
}

struct Parser {
    pub tokens: Vec<TokenInfo>,
    pub buf: Vec<u8>,
    pub line_num: usize,
    pub line_start: usize,
    pub token_is_tag: bool,
}

#[allow(clippy::while_let_on_iterator)]
pub(crate) fn tokenize(bytes: &[u8]) -> Result<Vec<TokenInfo>, Error> {
    let mut parser = Parser::new(bytes);
    let mut state = State::None;
    let mut iter = bytes.iter().enumerate().peekable();
    let mut last_ch = 0;

    'outer: while let Some((pos, &ch)) = iter.next() {
        match state {
            State::None => match ch {
                b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'$' => {
                    parser.push_byte(ch);
                }
                b'A'..=b'Z' => {
                    parser.push_byte(ch.to_ascii_lowercase());
                }
                b':' => {
                    if parser.is_token_start() {
                        parser.token_is_tag();
                    } else if parser.token_bytes().eq_ignore_ascii_case(b"text") {
                        state = State::MultiLine {
                            line_num: parser.line_num,
                            line_pos: pos - parser.line_start,
                        };
                        while let Some((pos, &ch)) = iter.next() {
                            if ch == b'\n' {
                                last_ch = b'\n';
                                parser.new_line(pos);
                                parser.reset_current_token();
                                continue 'outer;
                            }
                        }
                    } else {
                        return Err(Error::invalid_character(
                            parser.line_num,
                            pos - parser.line_start,
                            ch,
                        ));
                    }
                }
                b'"' => {
                    state = State::QuotedString {
                        line_num: parser.line_num,
                        line_pos: pos - parser.line_start,
                    };
                    parser.push_current_token(pos - 1);
                }
                b'{' => {
                    parser.push_token(Token::CurlyOpen, pos);
                }
                b'}' => {
                    parser.push_token(Token::CurlyClose, pos);
                }
                b';' => {
                    parser.push_token(Token::Semicolon, pos);
                }
                b',' => {
                    parser.push_token(Token::Comma, pos);
                }
                b'[' => {
                    parser.push_token(Token::BracketOpen, pos);
                }
                b']' => {
                    parser.push_token(Token::BracketClose, pos);
                }
                b'(' => {
                    parser.push_token(Token::ParenthesisOpen, pos);
                }
                b')' => {
                    parser.push_token(Token::ParenthesisClose, pos);
                }
                b'/' => {
                    if let Some((_, b'*')) = iter.next() {
                        last_ch = 0;
                        state = State::BracketComment {
                            line_num: parser.line_num,
                            line_pos: pos - parser.line_start,
                        };
                        parser.push_current_token(pos - 1);
                        continue;
                    } else {
                        return Err(Error::invalid_character(
                            parser.line_num,
                            pos - parser.line_start,
                            ch,
                        ));
                    }
                }
                b'#' => {
                    state = State::HashComment;
                    parser.push_current_token(pos - 1);
                }
                b'\n' => {
                    parser.push_current_token(pos - 1);
                    parser.new_line(pos);
                }
                b' ' | b'\t' | b'\r' => {
                    parser.push_current_token(pos - 1);
                }
                _ => {
                    return Err(Error::invalid_character(
                        parser.line_num,
                        pos - parser.line_start,
                        ch,
                    ));
                }
            },
            State::BracketComment { .. } => match ch {
                b'/' if last_ch == b'*' => {
                    state = State::None;
                }
                b'\n' => {
                    parser.new_line(pos);
                }
                _ => (),
            },
            State::HashComment => {
                if ch == b'\n' {
                    state = State::None;
                    parser.new_line(pos);
                }
            }
            State::QuotedString { .. } => match ch {
                b'\\' => (),
                b'"' if last_ch != b'\\' => {
                    parser.push_string(pos);
                    state = State::None;
                }
                b'\n' => {
                    parser.new_line(pos);
                    parser.push_byte(b'\n');
                }
                _ => {
                    parser.push_byte(ch);
                }
            },
            State::MultiLine { .. } => match ch {
                b'.' if last_ch == b'\n' => {
                    match (iter.next(), iter.peek()) {
                        (Some((_, b'\r')), Some((_, b'\n'))) => {
                            last_ch = b'\n';
                            iter.next();
                        }
                        (Some((_, b'\n')), _) => {
                            last_ch = b'\n';
                        }
                        (Some((_, b'.')), _) => {
                            parser.push_byte(b'.');
                            last_ch = b'.';
                        }
                        (Some((_, &ch)), _) => {
                            last_ch = ch;
                            parser.push_byte(b'.');
                            parser.push_byte(ch);
                        }
                        _ => (),
                    }

                    if last_ch == b'\n' {
                        parser.push_string(pos);
                        parser.new_line(pos);
                        state = State::None;
                    }

                    continue;
                }
                b'\n' => {
                    parser.new_line(pos);
                    parser.push_byte(b'\n');
                }
                _ => {
                    parser.push_byte(ch);
                }
            },
        }

        last_ch = ch;
    }

    match state {
        State::BracketComment { line_num, line_pos } => Err(Error {
            line_num,
            line_pos,
            error_type: ErrorType::UnterminatedComment,
        }),
        State::QuotedString { line_num, line_pos } => Err(Error {
            line_num,
            line_pos,
            error_type: ErrorType::UnterminatedString,
        }),
        State::MultiLine { line_num, line_pos } => Err(Error {
            line_num,
            line_pos,
            error_type: ErrorType::UnterminatedMultiline,
        }),
        _ => Ok(parser.finish()),
    }
}

impl Error {
    pub fn invalid_character(line_num: usize, line_pos: usize, ch: u8) -> Self {
        Error {
            line_num,
            line_pos,
            error_type: ErrorType::InvalidCharacter(ch),
        }
    }
}

impl Parser {
    pub fn new(bytes: &[u8]) -> Self {
        Parser {
            tokens: Vec::with_capacity(64),
            buf: Vec::with_capacity(bytes.len() / 2),
            line_num: 1,
            line_start: 0,
            token_is_tag: false,
        }
    }

    pub fn push_current_token(&mut self, pos: usize) {
        if !self.buf.is_empty() {
            let mut word = std::str::from_utf8(&self.buf).unwrap();
            let token = if let Some(word) = WORDS.get(word) {
                if self.token_is_tag {
                    Token::Tag(*word)
                } else {
                    Token::Identifier(*word)
                }
            } else {
                let multiplier = match self.buf.last().unwrap() {
                    b'k' => 1024,
                    b'm' => 1048576,
                    b'g' => 1073741824,
                    _ => 1,
                };

                if multiplier > 1 && self.buf.len() > 1 {
                    word = std::str::from_utf8(&self.buf[..self.buf.len() - 1]).unwrap();
                }

                if let Ok(number) = word.parse::<usize>() {
                    Token::Number(number * multiplier)
                } else {
                    Token::Invalid(word.to_string())
                }
            };

            self.tokens.push(TokenInfo {
                token,
                line_num: self.line_num,
                line_pos: pos - self.line_start,
            });

            self.reset_current_token();
        }
    }

    #[inline(always)]
    pub fn reset_current_token(&mut self) {
        self.buf.clear();
        self.token_is_tag = false;
    }

    #[inline(always)]
    pub fn token_is_tag(&mut self) {
        self.token_is_tag = true;
    }

    pub fn push_token(&mut self, token: Token, pos: usize) {
        self.push_current_token(pos);
        self.tokens.push(TokenInfo {
            token,
            line_num: self.line_num,
            line_pos: pos - self.line_start,
        });
    }

    pub fn push_string(&mut self, pos: usize) {
        self.push_token(Token::String(self.buf.to_vec()), pos);
        self.buf.clear();
    }

    #[inline(always)]
    pub fn push_byte(&mut self, ch: u8) {
        self.buf.push(ch);
    }

    #[inline(always)]
    pub fn new_line(&mut self, pos: usize) {
        self.line_num += 1;
        self.line_start = pos;
    }

    #[inline(always)]
    pub fn is_token_start(&self) -> bool {
        self.buf.is_empty()
    }

    #[inline(always)]
    pub fn token_bytes(&self) -> &[u8] {
        &self.buf
    }

    #[inline(always)]
    pub fn finish(self) -> Vec<TokenInfo> {
        self.tokens
    }
}
