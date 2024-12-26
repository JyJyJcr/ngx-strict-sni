use core::str;

use fluent_uri::UriRef;

pub struct ParseHostHeaderError;
pub fn parse_host_header(host_header: &str) -> Result<(&str, Option<u16>), ParseHostHeaderError> {
    Ok(match host_header.rsplit_once(':') {
        Some((host, port_str)) => {
            if port_str.is_empty() {
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

// pub enum URI<'a> {
//     Absolute { scheme: &'a str },
// }

pub struct ParseRequestLineError;
pub fn parse_request_line(
    request_line: &str,
) -> Result<(&str, UriRef<&str>, Option<&str>), ParseRequestLineError> {
    let mut iter = request_line.split(' ').filter(|s| !s.is_empty());
    if let Some(method) = iter.next() {
        if let Some(uri) = iter.next() {
            let ver = iter.next();
            if let Ok(uri) = UriRef::parse(uri) {
                return Ok((method, uri, ver));
            }
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
