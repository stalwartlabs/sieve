/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

use sieve::{Context, runtime::Variable};

use super::transform;

const MAX_URI_LEN: usize = u16::MAX as usize - 1;
const MAX_SCHEME_LEN: usize = 64;
const MAX_AUTHORITY_COLONS: u32 = 8;

#[derive(Debug, Default, PartialEq, Eq)]
struct Uri<'s> {
    scheme: Option<&'s str>,
    authority: &'s str,
    path_and_query: &'s str,
    query: Option<&'s str>,
}

#[derive(Clone, Copy)]
enum UriPart {
    Scheme,
    Host,
    SchemeHost,
    Path,
    Port,
    Query,
    PathQuery,
    Authority,
}

impl UriPart {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "scheme" => Some(Self::Scheme),
            "host" => Some(Self::Host),
            "scheme_host" => Some(Self::SchemeHost),
            "path" => Some(Self::Path),
            "port" => Some(Self::Port),
            "query" => Some(Self::Query),
            "path_query" => Some(Self::PathQuery),
            "authority" => Some(Self::Authority),
            _ => None,
        }
    }
}

fn is_uri_char(b: u8) -> bool {
    matches!(
        b,
        b'!' | b'#'
            | b'$'
            | b'&'
            | b'\''
            | b'('..=b';'
            | b'='
            | b'?'
            | b'@'..=b'['
            | b']'
            | b'_'
            | b'a'..=b'z'
            | b'~'
    )
}

fn is_scheme_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.' | b'~')
}

fn is_path_char(b: u8) -> bool {
    matches!(
        b,
        0x21 | 0x22 | 0x24..=0x3B | 0x3D | 0x40..=0x5F | 0x61..=0x7E | 0x80..=0xFF
    )
}

fn is_query_char(b: u8) -> bool {
    matches!(b, 0x21 | 0x24..=0x3B | 0x3D | 0x3F..=0x7E | 0x80..=0xFF)
}

fn authority_end(s: &str) -> Option<usize> {
    let mut colon_count = 0;
    let mut start_bracket = false;
    let mut end_bracket = false;
    let mut has_percent = false;
    let mut end = s.len();
    let mut at_sign_pos = None;

    for (pos, b) in s.bytes().enumerate() {
        match b {
            b'/' | b'?' | b'#' => {
                end = pos;
                break;
            }
            b':' => {
                if colon_count >= MAX_AUTHORITY_COLONS {
                    return None;
                }
                colon_count += 1;
            }
            b'[' => {
                if has_percent || start_bracket {
                    return None;
                }
                start_bracket = true;
            }
            b']' => {
                if !start_bracket || end_bracket {
                    return None;
                }
                end_bracket = true;
                colon_count = 0;
                has_percent = false;
            }
            b'@' => {
                at_sign_pos = Some(pos);
                colon_count = 0;
                has_percent = false;
            }
            b'%' => has_percent = true,
            b if is_uri_char(b) => {}
            _ => return None,
        }
    }

    (!s.is_empty()
        && start_bracket == end_bracket
        && colon_count <= 1
        && (end == 0 || at_sign_pos != Some(end - 1))
        && !has_percent)
        .then_some(end)
}

fn parse_path_and_query(s: &str) -> Option<(&str, Option<&str>)> {
    if !s.starts_with(['/', '?', '#']) {
        return None;
    }

    let mut bytes = s.bytes().enumerate();
    let mut end = s.len();
    let mut query_start = None;
    for (pos, b) in bytes.by_ref() {
        match b {
            b'?' => {
                query_start = Some(pos);
                break;
            }
            b'#' => {
                end = pos;
                break;
            }
            b if is_path_char(b) => {}
            _ => return None,
        }
    }
    if query_start.is_some() {
        for (pos, b) in bytes {
            match b {
                b'#' => {
                    end = pos;
                    break;
                }
                b if is_query_char(b) => {}
                _ => return None,
            }
        }
    }

    let data = s.get(..end)?;
    let query = match query_start {
        Some(pos) => Some(data.get(pos + 1..)?),
        None => None,
    };
    Some((data, query))
}

fn split_scheme(s: &str) -> Option<(Option<&str>, &str)> {
    for (prefix, scheme) in [("http://", "http"), ("https://", "https")] {
        if let Some((head, rest)) = s.split_at_checked(prefix.len())
            && head.eq_ignore_ascii_case(prefix)
        {
            return Some((Some(scheme), rest));
        }
    }

    if s.len() > 3 {
        for (pos, b) in s.bytes().enumerate() {
            match b {
                b':' => {
                    let (scheme, rest) = s.split_at(pos);
                    let Some(rest) = rest.strip_prefix("://") else {
                        break;
                    };
                    if pos > MAX_SCHEME_LEN {
                        return None;
                    }
                    return Some((Some(scheme), rest));
                }
                b if is_scheme_char(b) => {}
                _ => break,
            }
        }
    }

    Some((None, s))
}

impl<'s> Uri<'s> {
    fn parse(s: &'s str) -> Option<Self> {
        if s.is_empty() || s.len() > MAX_URI_LEN {
            return None;
        }
        if s == "/" || s == "*" {
            return Some(Uri {
                path_and_query: s,
                ..Default::default()
            });
        }
        if s.len() == 1 {
            return (authority_end(s)? == s.len()).then_some(Uri {
                authority: s,
                ..Default::default()
            });
        }
        if s.starts_with('/') {
            let (path_and_query, query) = parse_path_and_query(s)?;
            return Some(Uri {
                path_and_query,
                query,
                ..Default::default()
            });
        }

        let (scheme, rest) = split_scheme(s)?;
        let end = authority_end(rest)?;
        let Some(scheme) = scheme else {
            return (end == rest.len()).then_some(Uri {
                authority: rest,
                ..Default::default()
            });
        };
        if end == 0 {
            return None;
        }
        let (authority, tail) = rest.split_at(end);
        let (path_and_query, query) = if tail.is_empty() {
            ("/", None)
        } else {
            parse_path_and_query(tail)?
        };
        Some(Uri {
            scheme: Some(scheme),
            authority,
            path_and_query,
            query,
        })
    }

    fn authority(&self) -> Option<&'s str> {
        (!self.authority.is_empty()).then_some(self.authority)
    }

    fn host(&self) -> Option<&'s str> {
        let authority = self.authority()?;
        let host_port = authority.rsplit('@').next().unwrap_or(authority);
        if host_port.starts_with('[') {
            host_port.find(']').and_then(|pos| host_port.get(..=pos))
        } else {
            host_port.split(':').next()
        }
    }

    fn port(&self) -> Option<u16> {
        let authority = self.authority()?;
        let (_, port) = authority.rsplit_once(':')?;
        port.parse().ok()
    }

    fn path(&self) -> &'s str {
        if self.path_and_query.is_empty() && self.scheme.is_none() {
            return "";
        }
        let path = match self.query {
            Some(query) => self
                .path_and_query
                .get(..self.path_and_query.len() - query.len() - 1)
                .unwrap_or_default(),
            None => self.path_and_query,
        };
        if path.is_empty() { "/" } else { path }
    }

    fn path_and_query(&self) -> Option<Variable<'s>> {
        if self.scheme.is_none() && !self.authority.is_empty() {
            return None;
        }
        Some(if self.path_and_query.is_empty() {
            Variable::borrowed("/")
        } else if self.path_and_query.starts_with(['/', '*']) {
            Variable::borrowed(self.path_and_query)
        } else {
            Variable::from(format!("/{}", self.path_and_query))
        })
    }

    fn part(&self, part: UriPart) -> Option<Variable<'s>> {
        match part {
            UriPart::Scheme => self.scheme.map(Variable::borrowed),
            UriPart::Host => self.host().map(Variable::borrowed),
            UriPart::SchemeHost => {
                let (scheme, host) = (self.scheme?, self.host()?);
                Some(Variable::from(format!("{scheme}://{host}")))
            }
            UriPart::Path => Some(Variable::borrowed(self.path())),
            UriPart::Port => self.port().map(|port| Variable::Integer(i64::from(port))),
            UriPart::Query => self.query.map(Variable::borrowed),
            UriPart::PathQuery => self.path_and_query(),
            UriPart::Authority => self.authority().map(Variable::borrowed),
        }
    }
}

pub fn fn_uri_part<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    let [value, part, ..] = args else {
        return Variable::empty();
    };
    let Some(part) = UriPart::parse(&part.to_string()) else {
        return transform(value, |_| Variable::empty());
    };
    transform(value, |uri| {
        Uri::parse(uri)
            .and_then(|uri| uri.part(part))
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use sieve::runtime::Variable;

    use super::*;
    use crate::functions::tests::{call, list, s};

    fn part(uri: &'static str, name: &'static str) -> Variable<'static> {
        call(fn_uri_part, &[s(uri), s(name)])
    }

    #[test]
    fn absolute_uris() {
        let uri = "https://user:pw@Example.org:8443/a/b?x=1&y=2#frag";
        assert_eq!(part(uri, "scheme"), s("https"));
        assert_eq!(part(uri, "host"), s("Example.org"));
        assert_eq!(part(uri, "scheme_host"), s("https://Example.org"));
        assert_eq!(part(uri, "path"), s("/a/b"));
        assert_eq!(part(uri, "port"), Variable::Integer(8443));
        assert_eq!(part(uri, "query"), s("x=1&y=2"));
        assert_eq!(part(uri, "path_query"), s("/a/b?x=1&y=2"));
        assert_eq!(part(uri, "authority"), s("user:pw@Example.org:8443"));
        assert_eq!(part(uri, "unknown"), s(""));
    }

    #[test]
    fn scheme_variants() {
        assert_eq!(part("HTTP://example.org", "scheme"), s("http"));
        assert_eq!(part("HTTP://example.org", "path"), s("/"));
        assert_eq!(part("HTTP://example.org", "path_query"), s("/"));
        assert_eq!(part("ftp://files.example.org/pub", "scheme"), s("ftp"));
        assert_eq!(part("ftp://files.example.org/pub", "path"), s("/pub"));
        assert_eq!(part("http://example.org?q", "path"), s("/"));
        assert_eq!(part("http://example.org?q", "query"), s("q"));
        assert_eq!(part("http://example.org?q", "path_query"), s("/?q"));
        assert_eq!(part("http://[::1]:8080/", "host"), s("[::1]"));
        assert_eq!(part("http://[::1]:8080/", "port"), Variable::Integer(8080));
        assert_eq!(part("http://[::1]/", "port"), s(""));
    }

    #[test]
    fn relative_and_authority_forms() {
        assert_eq!(part("/path?q=1", "path"), s("/path"));
        assert_eq!(part("/path?q=1", "query"), s("q=1"));
        assert_eq!(part("/path?q=1", "host"), s(""));
        assert_eq!(part("/path?q=1", "path_query"), s("/path?q=1"));
        assert_eq!(part("example.org:25", "host"), s("example.org"));
        assert_eq!(part("example.org:25", "port"), Variable::Integer(25));
        assert_eq!(part("example.org:25", "path"), s(""));
        assert_eq!(part("example.org:25", "path_query"), s(""));
        assert_eq!(part("example.org:25", "scheme"), s(""));
        assert_eq!(part("*", "path"), s("*"));
        assert_eq!(part("a", "authority"), s("a"));
    }

    #[test]
    fn invalid_uris() {
        for uri in [
            "",
            "http://",
            "http:///path",
            "http://exa mple.org/",
            "http://a:1:2/",
            "http://user@/",
            "http://[::1/",
            "http://ex%41mple.org/",
            "example.org/path",
            "/pa th",
            "http://example.org/a b",
        ] {
            assert_eq!(part(uri, "host"), s(""), "{uri}");
            assert_eq!(part(uri, "path"), s(""), "{uri}");
        }
    }

    #[test]
    fn arrays() {
        assert_eq!(
            call(
                fn_uri_part,
                &[list(&["https://a.org/x", "mailto:b"]), s("host")]
            ),
            list(&["a.org", "mailto"])
        );
        assert_eq!(
            call(fn_uri_part, &[list(&["https://a.org/x"]), s("bogus")]),
            list(&[""])
        );
    }

    #[test]
    fn parse_matches_http_crate_layout() {
        assert_eq!(
            Uri::parse("https://a.org/p?q#f"),
            Some(Uri {
                scheme: Some("https"),
                authority: "a.org",
                path_and_query: "/p?q",
                query: Some("q"),
            })
        );
    }
}
