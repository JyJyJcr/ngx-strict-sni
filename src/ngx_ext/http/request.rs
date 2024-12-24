use ngx::{core::NgxStr, http::Request};

use crate::ngx_ext::str::try_to_ref;

use super::HTTPModule;

pub trait RequestExt {
    fn host_header<'a>(&'a self) -> Option<&'a NgxStr>;
    fn request_line<'a>(&'a self) -> Option<&'a NgxStr>;

    #[allow(dead_code)]
    fn main_conf<'a, M: HTTPModule>(&'a self) -> Option<&'a M::MainConf>;
    #[allow(dead_code)]
    fn srv_conf<'a, M: HTTPModule>(&'a self) -> Option<&'a M::SrvConf>;
    #[allow(dead_code)]
    fn loc_conf<'a, M: HTTPModule>(&'a self) -> Option<&'a M::LocConf>;

    #[allow(dead_code)]
    fn get_ctx<'a, M: HTTPModule>(&'a self) -> Option<&'a M::Ctx>;
    #[allow(dead_code)]
    fn get_ctx_mut<'a, M: HTTPModule>(&'a mut self) -> Option<&'a mut M::Ctx>;
    #[allow(dead_code)]
    fn set_ctx<'a, M: HTTPModule>(&'a mut self, ctx: &'a M::Ctx);
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

    fn main_conf<'a, M: HTTPModule>(&'a self) -> Option<&'a M::MainConf> {
        self.get_module_main_conf::<M::MainConf>(M::module_ref())
    }
    fn srv_conf<'a, M: HTTPModule>(&'a self) -> Option<&'a M::SrvConf> {
        self.get_module_srv_conf::<M::SrvConf>(M::module_ref())
    }
    fn loc_conf<'a, M: HTTPModule>(&'a self) -> Option<&'a M::LocConf> {
        self.get_module_loc_conf::<M::LocConf>(M::module_ref())
    }

    fn get_ctx<'a, M: HTTPModule>(&'a self) -> Option<&'a M::Ctx> {
        self.get_module_ctx::<M::Ctx>(M::module_ref())
    }
    fn get_ctx_mut<'a, M: HTTPModule>(&'a mut self) -> Option<&'a mut M::Ctx> {
        let p = unsafe { *self.get_inner().ctx.add(M::module_ref().ctx_index) };
        let ctx = p.cast::<M::Ctx>();
        unsafe { ctx.as_mut() }
    }
    fn set_ctx<'a, M: HTTPModule>(&'a mut self, ctx: &'a M::Ctx) {
        self.set_module_ctx(ctx as *const _ as *mut _, M::module_ref())
    }
}
