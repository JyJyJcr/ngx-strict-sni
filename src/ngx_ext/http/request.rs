use ngx::{
    core::NgxStr,
    ffi::{ngx_connection_t, ngx_inet_get_port},
    http::Request,
};

use crate::ngx_ext::str::try_to_ref;

use super::HTTPModule;

pub trait RequestExt {
    // note: you can elide lifetime parameter if the returned ref's lifetime is same to self.
    // https://doc.rust-lang.org/nomicon/lifetime-elision.html
    fn host_header(&self) -> Option<&NgxStr>;
    fn request_line(&self) -> Option<&NgxStr>;
    fn connection(&self) -> Option<&Connection>;
    fn connection_mut(&mut self) -> Option<&mut Connection>;

    fn main_conf<M: HTTPModule>(&self) -> Option<&M::MainConf>;
    fn srv_conf<M: HTTPModule>(&self) -> Option<&M::SrvConf>;
    fn loc_conf<M: HTTPModule>(&self) -> Option<&M::LocConf>;
    fn get_ctx<M: HTTPModule>(&self) -> Option<&M::Ctx>;
    fn get_ctx_mut<M: HTTPModule>(&mut self) -> Option<&mut M::Ctx>;
    fn set_ctx<M: HTTPModule>(&mut self, ctx: &M::Ctx);

    fn is_internal(&self) -> bool;
}

impl RequestExt for Request {
    fn host_header(&self) -> Option<&NgxStr> {
        let inner = self.get_inner();
        if let Some(elt) = unsafe { inner.headers_in.host.as_ref() } {
            try_to_ref(elt.value)
        } else {
            None
        }
    }
    fn request_line(&self) -> Option<&NgxStr> {
        let inner = self.get_inner();
        try_to_ref(inner.request_line)
    }

    fn main_conf<M: HTTPModule>(&self) -> Option<&M::MainConf> {
        self.get_module_main_conf::<M::MainConf>(M::module_ref())
    }
    fn srv_conf<M: HTTPModule>(&self) -> Option<&M::SrvConf> {
        self.get_module_srv_conf::<M::SrvConf>(M::module_ref())
    }
    fn loc_conf<M: HTTPModule>(&self) -> Option<&M::LocConf> {
        self.get_module_loc_conf::<M::LocConf>(M::module_ref())
    }

    fn get_ctx<M: HTTPModule>(&self) -> Option<&M::Ctx> {
        self.get_module_ctx::<M::Ctx>(M::module_ref())
    }
    fn get_ctx_mut<M: HTTPModule>(&mut self) -> Option<&mut M::Ctx> {
        let p = unsafe { *self.get_inner().ctx.add(M::module_ref().ctx_index) };
        let ctx = p.cast::<M::Ctx>();
        unsafe { ctx.as_mut() }
    }
    fn set_ctx<M: HTTPModule>(&mut self, ctx: &M::Ctx) {
        self.set_module_ctx(ctx as *const _ as *mut _, M::module_ref())
    }
    fn connection(&self) -> Option<&Connection> {
        let p = self.connection();
        if p.is_null() {
            None
        } else {
            Some(unsafe { &*p.cast::<Connection>() })
        }
    }

    fn connection_mut(&mut self) -> Option<&mut Connection> {
        let p = self.connection();
        if p.is_null() {
            None
        } else {
            Some(unsafe { &mut *p.cast::<Connection>() })
        }
    }

    fn is_internal(&self) -> bool {
        self.get_inner().internal() != 0
    }
}

pub struct Connection(ngx_connection_t);
impl Connection {
    pub fn local_port(&self) -> Option<u16> {
        if let Some(addr) = unsafe { self.0.local_sockaddr.as_mut() } {
            // ngx_inet_get_port is implemented without the use of mutability, so no problem
            let p = unsafe { ngx_inet_get_port(addr) };
            if p != 0 {
                return Some(p);
            }
        }
        None
    }
}
