use ngx::{
    core::NgxStr,
    ffi::{ngx_connection_t, ngx_inet_get_port},
    http::{HttpModule, HttpModuleSkel, InitConfSetting, MergeConfSetting, Request},
    module::Module,
};

use crate::ngx_ext::str::try_to_ref;

pub trait RequestExt {
    // note: you can elide lifetime parameter if the returned ref's lifetime is same to self.
    // https://doc.rust-lang.org/nomicon/lifetime-elision.html
    fn host_header(&self) -> Option<&NgxStr>;
    fn request_line(&self) -> Option<&NgxStr>;
    fn connection(&self) -> Option<&Connection>;

    fn main_conf<M: HttpModule>(&self) -> Option<&<M::MainConfSetting as InitConfSetting>::Conf>;
    fn srv_conf<M: HttpModule>(&self) -> Option<&<M::SrvConfSetting as MergeConfSetting>::Conf>;
    fn loc_conf<M: HttpModule>(&self) -> Option<&<M::LocConfSetting as MergeConfSetting>::Conf>;
    fn get_ctx<M: HttpModule>(&self) -> Option<&M::Ctx>;
    fn set_ctx<M: HttpModule>(&self, ctx: &M::Ctx);

    fn is_internal(&self) -> bool;
}

impl RequestExt for Request {
    fn host_header(&self) -> Option<&NgxStr> {
        let inner = self.get_inner();
        if let Some(elt) = unsafe { inner.headers_in.host.as_ref() } {
            Some(try_to_ref(elt.value))
        } else {
            None
        }
    }
    fn request_line(&self) -> Option<&NgxStr> {
        let inner = self.get_inner();
        Some(try_to_ref(inner.request_line))
    }

    fn main_conf<M: HttpModule>(&self) -> Option<&<M::MainConfSetting as InitConfSetting>::Conf> {
        self.get_module_main_conf::<<M::MainConfSetting as InitConfSetting>::Conf>(
            unsafe { HttpModuleSkel::<M>::SELF.to_ref() }.inner(),
        )
    }
    fn srv_conf<M: HttpModule>(&self) -> Option<&<M::SrvConfSetting as MergeConfSetting>::Conf> {
        self.get_module_srv_conf::<<M::SrvConfSetting as MergeConfSetting>::Conf>(
            unsafe { HttpModuleSkel::<M>::SELF.to_ref() }.inner(),
        )
    }
    fn loc_conf<M: HttpModule>(&self) -> Option<&<M::LocConfSetting as MergeConfSetting>::Conf> {
        self.get_module_loc_conf::<<M::LocConfSetting as MergeConfSetting>::Conf>(
            unsafe { HttpModuleSkel::<M>::SELF.to_ref() }.inner(),
        )
    }

    fn get_ctx<M: HttpModule>(&self) -> Option<&M::Ctx> {
        self.get_module_ctx::<M::Ctx>(unsafe { HttpModuleSkel::<M>::SELF.to_ref() }.inner())
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
    fn set_ctx<M: HttpModule>(&self, ctx: &M::Ctx) {
        self.set_module_ctx(
            ctx as *const _ as *mut _,
            unsafe { HttpModuleSkel::<M>::SELF.to_ref() }.inner(),
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
