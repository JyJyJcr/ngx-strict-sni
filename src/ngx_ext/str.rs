use ngx::{core::NgxStr, ffi::ngx_str_t};

pub fn try_to_ref<'a>(raw: ngx_str_t) -> &'a NgxStr {
    if raw.len == 0 {
        let r: &[u8] = &[];
        <&NgxStr>::from(r)
    } else {
        unsafe { NgxStr::from_ngx_str(raw) }
    }
}
