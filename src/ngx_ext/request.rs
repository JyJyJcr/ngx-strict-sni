use ngx::{core::NgxStr, http::Request};

use super::str::try_to_ref;
pub trait RequestExt {
    fn host_header<'a>(&'a self) -> Option<&'a NgxStr>;
    fn request_line<'a>(&'a self) -> Option<&'a NgxStr>;
}

impl RequestExt for Request {
    fn host_header<'a>(&'a self) -> Option<&'a NgxStr> {
        let inner = self.get_inner();
        if let Some(elt) = unsafe { inner.headers_in.host.as_ref() } {
            try_to_ref(elt.value)
        } else {
            None
        }
    }
    fn request_line<'a>(&'a self) -> Option<&'a NgxStr> {
        let inner = self.get_inner();
        try_to_ref(inner.request_line)
    }
}
