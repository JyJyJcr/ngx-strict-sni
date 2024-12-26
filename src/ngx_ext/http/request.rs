use ngx::{
    core::NgxStr,
    ffi::{ngx_connection_t, ngx_inet_get_port},
    http::Request,
};

use crate::ngx_ext::{str::try_to_ref, NgxModule};

use super::NgxHttpModuleImpl;

pub trait RequestExt {
    // note: you can elide lifetime parameter if the returned ref's lifetime is same to self.
    // https://doc.rust-lang.org/nomicon/lifetime-elision.html
    fn host_header(&self) -> Option<&NgxStr>;
    fn request_line(&self) -> Option<&NgxStr>;
    fn connection(&self) -> Option<&Connection>;

    fn main_conf<M: NgxHttpModuleImpl>(&self) -> Option<&M::MainConf>;
    fn srv_conf<M: NgxHttpModuleImpl>(&self) -> Option<&M::SrvConf>;
    fn loc_conf<M: NgxHttpModuleImpl>(&self) -> Option<&M::LocConf>;
    fn get_ctx<M: NgxHttpModuleImpl>(&self) -> Option<&M::Ctx>;
    fn set_ctx<M: NgxHttpModuleImpl>(&self, ctx: &M::Ctx);

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

    fn main_conf<I: NgxHttpModuleImpl>(&self) -> Option<&I::MainConf> {
        self.get_module_main_conf::<I::MainConf>(<I::Module as NgxModule>::module())
    }
    fn srv_conf<I: NgxHttpModuleImpl>(&self) -> Option<&I::SrvConf> {
        self.get_module_srv_conf::<I::SrvConf>(<I::Module as NgxModule>::module())
    }
    fn loc_conf<I: NgxHttpModuleImpl>(&self) -> Option<&I::LocConf> {
        self.get_module_loc_conf::<I::LocConf>(<I::Module as NgxModule>::module())
    }

    fn get_ctx<I: NgxHttpModuleImpl>(&self) -> Option<&I::Ctx> {
        self.get_module_ctx::<I::Ctx>(<I::Module as NgxModule>::module())
    }
    // fn get_ctx_mut<I: NgxHttpModuleImpl>(&mut self) -> Option<&mut I::Ctx> {
    //     let p = unsafe {
    //         *self
    //             .get_inner()
    //             .ctx
    //             .add(<I::Module as NgxModule>::module().ctx_index)
    //     };
    //     let ctx = p.cast::<I::Ctx>();
    //     unsafe { ctx.as_mut() }
    // }
    fn set_ctx<I: NgxHttpModuleImpl>(&self, ctx: &I::Ctx) {
        self.set_module_ctx(
            ctx as *const _ as *mut _,
            <I::Module as NgxModule>::module(),
        )
    }
    fn connection(&self) -> Option<&Connection> {
        let p = self.connection();
        if p.is_null() {
            None
        } else {
            Some(unsafe { &*p.cast::<Connection>() })
        }
    }

    // fn connection_mut(&mut self) -> Option<&mut Connection> {
    //     let p = self.connection();
    //     if p.is_null() {
    //         None
    //     } else {
    //         Some(unsafe { &mut *p.cast::<Connection>() })
    //     }
    // }

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
