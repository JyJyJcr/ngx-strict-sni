use ngx::{core::NgxStr, ffi::ngx_str_t};

pub fn try_to_ref<'a>(raw: ngx_str_t) -> Option<&'a NgxStr> {
    if raw.data.is_null() {
        None
    } else {
        Some(unsafe { NgxStr::from_ngx_str(raw) })
    }
}
