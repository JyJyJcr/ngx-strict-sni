use std::ptr::slice_from_raw_parts;

use ngx::{
    ffi::{
        ngx_conf_t, ngx_http_get_flushed_variable, ngx_http_get_variable_index, ngx_int_t,
        ngx_str_t, ngx_uint_t, NGX_ERROR,
    },
    http::Request,
};

#[derive(Debug, Clone, Copy)]
pub struct VariableHook(ngx_uint_t);

pub struct VariableHookGetError;

impl VariableHook {
    pub fn hook(cf: &ngx_conf_t, name: &ngx_str_t) -> Result<Self, VariableHookGetError> {
        let r = unsafe {
            ngx_http_get_variable_index(cf as *const _ as *mut _, name as *const _ as *mut _)
        };
        if r == NGX_ERROR as ngx_int_t {
            Err(VariableHookGetError)
        } else {
            Ok(VariableHook(r as ngx_uint_t))
        }
    }
    pub fn get<'a>(&self, req: &'a Request) -> Option<&'a [u8]> {
        let r =
            unsafe { ngx_http_get_flushed_variable(req.get_inner() as *const _ as *mut _, self.0) };
        if let Some(v) = unsafe { r.as_ref() } {
            if v.not_found() == 0 {
                let ptr = slice_from_raw_parts(v.data, v.len() as usize);
                if let Some(slice) = unsafe { ptr.as_ref() } {
                    return Some(slice);
                }
            }
        }
        None
    }
}

// fn solve_variable_ref_mut<'a>(r: &VariableRef,req:&'a mut Request)->Option<&'a mut [u8]>{
//     let r = unsafe { ngx_http_get_flushed_variable( req.get_inner() as *const _ as *mut _, r.0) };
//     if let Some(v) =unsafe{r.as_ref()} {
//         if v.not_found() == 0 {
//             let ptr=slice_from_raw_parts(v.data, v.len() as usize);
//             if let Some(slice)=unsafe{ptr.as_ref()}{
//                 return Some(slice)
//             }
//         }
//     }
//     None
// }
