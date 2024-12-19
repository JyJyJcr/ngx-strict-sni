use std::{ptr::slice_from_raw_parts, str};

use ngx::{ffi::ngx_str_t, http::Request};

pub fn to_rust_str_ref<'a>(ngx_str: &'a ngx_str_t) -> Option<&'a str> {
    if !ngx_str.data.is_null() {
        let ptr = slice_from_raw_parts(ngx_str.data, ngx_str.len);
        if let Some(slice) = unsafe { ptr.as_ref() } {
            return str::from_utf8(slice).ok();
        }
    }
    None
}

pub fn get_host_header_str<'a>(request: &'a Request) -> Option<&'a str> {
    let inner = request.get_inner();
    if let Some(elt) = unsafe { inner.headers_in.host.as_ref() } {
        to_rust_str_ref(&elt.value)
    } else {
        None
    }
}
pub struct ParseHostHeaderError;
pub fn parse_host_header(host_header: &str) -> Result<(&str, Option<u16>), ParseHostHeaderError> {
    Ok(match host_header.rsplit_once(':') {
        Some((host, port_str)) => {
            if port_str == "" {
                (host, None)
            } else {
                (
                    host,
                    Some(port_str.parse().map_err(|_| ParseHostHeaderError)?),
                )
            }
        }
        None => (host_header, None),
    })
}
pub fn get_request_line_str<'a>(request: &'a Request) -> Option<&'a str> {
    let inner = request.get_inner();
    to_rust_str_ref(&inner.request_line)
}

pub enum URI<'a> {
    Absolute { scheme: &'a str },
}

pub struct ParseRequestLineError;
pub fn parse_request_line(
    request_line: &str,
) -> Result<(&str, &str, Option<&str>), ParseRequestLineError> {
    let mut iter = request_line.split(' ').filter(|s| !s.is_empty());
    if let Some(method) = iter.next() {
        if let Some(uri) = iter.next() {
            let ver = if let Some(http_ver) = iter.next() {
                Some(http_ver)
            } else {
                None
            };
            return Ok((method, uri, ver));
        }
    }
    Err(ParseRequestLineError)
}

#[cfg(test)]
mod test {
    #[test]
    fn split_test() {
        let rl = "GET     /efnepfnap     x";
        let v: Vec<&str> = rl.split(' ').filter(|s| !s.is_empty()).collect();
        println!("{:?}", v);
        //assert_eq!(v, [])
    }
}
