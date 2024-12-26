pub mod request;
pub mod variable;

use core::{
    ffi::{c_char, c_void},
    marker::PhantomData,
    ptr::addr_of,
    ptr::null_mut,
};

use ngx::{
    core::{Pool, Status, NGX_CONF_ERROR},
    ffi::{
        ngx_array_push, ngx_conf_t, ngx_http_core_module, ngx_http_handler_pt, ngx_http_module_t,
        ngx_http_phases, ngx_http_phases_NGX_HTTP_ACCESS_PHASE,
        ngx_http_phases_NGX_HTTP_CONTENT_PHASE, ngx_http_phases_NGX_HTTP_FIND_CONFIG_PHASE,
        ngx_http_phases_NGX_HTTP_LOG_PHASE, ngx_http_phases_NGX_HTTP_POST_ACCESS_PHASE,
        ngx_http_phases_NGX_HTTP_POST_READ_PHASE, ngx_http_phases_NGX_HTTP_POST_REWRITE_PHASE,
        ngx_http_phases_NGX_HTTP_PREACCESS_PHASE, ngx_http_phases_NGX_HTTP_PRECONTENT_PHASE,
        ngx_http_phases_NGX_HTTP_REWRITE_PHASE, ngx_http_phases_NGX_HTTP_SERVER_REWRITE_PHASE,
        ngx_http_request_s, ngx_int_t, ngx_uint_t, NGX_HTTP_MODULE, NGX_RS_HTTP_LOC_CONF_OFFSET,
        NGX_RS_HTTP_MAIN_CONF_OFFSET, NGX_RS_HTTP_SRV_CONF_OFFSET,
    },
    http::{Merge, Request},
};

use super::{CommandCtx, NgxModule, NgxModuleImpl};

pub struct NgxHttpModule<I: NgxHttpModuleImpl> {
    ctx: ngx_http_module_t,
    __: PhantomData<I>,
}
impl<I: NgxHttpModuleImpl> NgxModuleImpl for NgxHttpModule<I> {
    type Module = I::Module;
    const MODULE_TYPE: ngx_uint_t = NGX_HTTP_MODULE as ngx_uint_t;
    type Ctx = ngx_http_module_t;
}

pub trait NgxHttpModuleImpl {
    type Module: NgxModule;
    // + 'static is unintended, but Request::get_loc_conf provide 'static T (why????)
    type MainConf: 'static;
    type SrvConf: 'static;
    type LocConf: 'static;
    type MainConfManager: ConfInitManager<Conf = Self::MainConf>;
    type SrvConfManager: ConfMergeManager<Conf = Self::SrvConf>;
    type LocConfManager: ConfMergeManager<Conf = Self::LocConf>;
    type Ctx;

    #[allow(unused_variables)]
    fn preconfiguration(cf: &mut ngx_conf_t) -> Result<(), Status> {
        Ok(())
    }
    #[allow(unused_variables)]
    fn postconfiguration(cf: &mut ngx_conf_t) -> Result<(), Status> {
        Ok(())
    }
}

pub trait ConfInitManager {
    type Conf;
    fn create(cf: &mut ngx_conf_t) -> Result<Self::Conf, ()>;
    fn init(cf: &mut ngx_conf_t, conf: &mut Self::Conf) -> Result<(), ()>;
}
pub trait ConfMergeManager {
    type Conf;
    fn create(cf: &mut ngx_conf_t) -> Result<Self::Conf, ()>;
    fn merge(cf: &mut ngx_conf_t, prev: &mut Self::Conf, conf: &mut Self::Conf) -> Result<(), ()>;
}

pub struct DefaultConfManager<T>(PhantomData<T>);
impl<T: Default> ConfInitManager for DefaultConfManager<T> {
    type Conf = T;
    fn create(_cf: &mut ngx_conf_t) -> Result<Self::Conf, ()> {
        Ok(Default::default())
    }

    fn init(_cf: &mut ngx_conf_t, _conf: &mut Self::Conf) -> Result<(), ()> {
        Ok(())
    }
}
impl<T: Default + Merge> ConfMergeManager for DefaultConfManager<T> {
    type Conf = T;
    fn create(_cf: &mut ngx_conf_t) -> Result<Self::Conf, ()> {
        Ok(Default::default())
    }

    fn merge(_cf: &mut ngx_conf_t, prev: &mut Self::Conf, conf: &mut Self::Conf) -> Result<(), ()> {
        conf.merge(prev).map_err(|_| ())
    }
}

pub const fn ngx_http_module_ctx<I: NgxHttpModuleImpl>() -> ngx_http_module_t {
    ngx_http_module_t {
        preconfiguration: Some(NgxHttpModuleImplCall::<I>::preconfiguration),
        postconfiguration: Some(NgxHttpModuleImplCall::<I>::postconfiguration),
        create_main_conf: Some(NgxHttpModuleImplCall::<I>::create_main_conf),
        init_main_conf: Some(NgxHttpModuleImplCall::<I>::init_main_conf),
        create_srv_conf: Some(NgxHttpModuleImplCall::<I>::create_srv_conf),
        merge_srv_conf: Some(NgxHttpModuleImplCall::<I>::merge_srv_conf),
        create_loc_conf: Some(NgxHttpModuleImplCall::<I>::create_loc_conf),
        merge_loc_conf: Some(NgxHttpModuleImplCall::<I>::merge_loc_conf),
    }
}

struct NgxHttpModuleImplCall<I: NgxHttpModuleImpl>(PhantomData<I>);
impl<I: NgxHttpModuleImpl> NgxHttpModuleImplCall<I> {
    /// # Safety
    ///
    /// Callers should provide valid non-null `ngx_conf_t` arguments. Implementers must
    /// guard against null inputs or risk runtime errors.
    unsafe extern "C" fn preconfiguration(cf: *mut ngx_conf_t) -> ngx_int_t {
        if let Some(cf) = cf.as_mut() {
            I::preconfiguration(cf).err().unwrap_or(Status::NGX_OK)
        } else {
            Status::NGX_ERROR
        }
        .into()
    }

    /// # Safety
    ///
    /// Callers should provide valid non-null `ngx_conf_t` arguments. Implementers must
    /// guard against null inputs or risk runtime errors.
    unsafe extern "C" fn postconfiguration(cf: *mut ngx_conf_t) -> ngx_int_t {
        if let Some(cf) = cf.as_mut() {
            I::postconfiguration(cf).err().unwrap_or(Status::NGX_OK)
        } else {
            Status::NGX_ERROR
        }
        .into()
    }

    /// # Safety
    ///
    /// Callers should provide valid non-null `ngx_conf_t` arguments. Implementers must
    /// guard against null inputs or risk runtime errors.
    unsafe extern "C" fn create_main_conf(cf: *mut ngx_conf_t) -> *mut c_void {
        // if failed to alloc, it return nullptr
        // and create_* ret nullptr mean fail
        // so here directly return
        if let Some(cf) = cf.as_mut() {
            if let Some(pool) = cf.pool.as_mut() {
                let mut pool = Pool::from_ngx_pool(pool);
                if let Ok(conf) = I::MainConfManager::create(cf) {
                    return pool.allocate(conf) as *mut c_void;
                }
            }
        }
        null_mut()
    }

    /// # Safety
    ///
    /// Callers should provide valid non-null `ngx_conf_t` arguments. Implementers must
    /// guard against null inputs or risk runtime errors.
    unsafe extern "C" fn init_main_conf(cf: *mut ngx_conf_t, conf: *mut c_void) -> *mut c_char {
        if let Some(cf) = cf.as_mut() {
            if let Some(conf) = (conf as *mut I::MainConf).as_mut() {
                if let Ok(_) = I::MainConfManager::init(cf, conf) {
                    return null_mut();
                }
            }
        }
        NGX_CONF_ERROR as _
    }

    /// # Safety
    ///
    /// Callers should provide valid non-null `ngx_conf_t` arguments. Implementers must
    /// guard against null inputs or risk runtime errors.
    unsafe extern "C" fn create_srv_conf(cf: *mut ngx_conf_t) -> *mut c_void {
        // if failed to alloc, it return nullptr
        // and create_* ret nullptr mean fail
        // so here directly return
        if let Some(cf) = cf.as_mut() {
            if let Some(pool) = cf.pool.as_mut() {
                let mut pool = Pool::from_ngx_pool(pool);
                if let Ok(conf) = I::SrvConfManager::create(cf) {
                    return pool.allocate(conf) as *mut c_void;
                }
            }
        }
        null_mut()
    }

    /// # Safety
    ///
    /// Callers should provide valid non-null `ngx_conf_t` arguments. Implementers must
    /// guard against null inputs or risk runtime errors.
    unsafe extern "C" fn merge_srv_conf(
        cf: *mut ngx_conf_t,
        prev: *mut c_void,
        conf: *mut c_void,
    ) -> *mut c_char {
        if let Some(cf) = cf.as_mut() {
            if let Some(prev) = (prev as *mut I::SrvConf).as_mut() {
                if let Some(conf) = (conf as *mut I::SrvConf).as_mut() {
                    if let Ok(_) = I::SrvConfManager::merge(cf, prev, conf) {
                        return null_mut();
                    }
                }
            }
        }
        NGX_CONF_ERROR as _
    }

    /// # Safety
    ///
    /// Callers should provide valid non-null `ngx_conf_t` arguments. Implementers must
    /// guard against null inputs or risk runtime errors.
    unsafe extern "C" fn create_loc_conf(cf: *mut ngx_conf_t) -> *mut c_void {
        // if failed to alloc, it return nullptr
        // and create_* ret nullptr mean fail
        // so here directly return
        if let Some(cf) = cf.as_mut() {
            if let Some(pool) = cf.pool.as_mut() {
                let mut pool = Pool::from_ngx_pool(pool);
                if let Ok(conf) = I::LocConfManager::create(cf) {
                    return pool.allocate(conf) as *mut c_void;
                }
            }
        }
        null_mut()
    }

    /// # Safety
    ///
    /// Callers should provide valid non-null `ngx_conf_t` arguments. Implementers must
    /// guard against null inputs or risk runtime errors.
    unsafe extern "C" fn merge_loc_conf(
        cf: *mut ngx_conf_t,
        prev: *mut c_void,
        conf: *mut c_void,
    ) -> *mut c_char {
        if let Some(cf) = cf.as_mut() {
            if let Some(prev) = (prev as *mut I::LocConf).as_mut() {
                if let Some(conf) = (conf as *mut I::LocConf).as_mut() {
                    if let Ok(_) = I::LocConfManager::merge(cf, prev, conf) {
                        return null_mut();
                    }
                }
            }
        }
        NGX_CONF_ERROR as _
    }
}

// impl<M: NgxHttpModuleImpl> HTTPModule for HTTPModuleWrapper<M> {
//     type MainConf = <M as NgxHttpModuleImpl>::MainConf;

//     type SrvConf = <M as NgxHttpModuleImpl>::SrvConf;

//     type LocConf = <M as NgxHttpModuleImpl>::LocConf;

//     unsafe extern "C" fn preconfiguration(cf: *mut ngx_conf_t) -> ngx_int_t {
//         if let Some(cf) = cf.as_mut() {
//             M::preconfiguration(cf).err().unwrap_or(Status::NGX_OK)
//         } else {
//             Status::NGX_ERROR
//         }
//         .into()
//     }

//     unsafe extern "C" fn postconfiguration(cf: *mut ngx_conf_t) -> ngx_int_t {
//         if let Some(cf) = cf.as_mut() {
//             M::postconfiguration(cf).err().unwrap_or(Status::NGX_OK)
//         } else {
//             Status::NGX_ERROR
//         }
//         .into()
//     }
// }
pub struct LocCtx<I: NgxHttpModuleImpl>(PhantomData<I>);
pub struct SrvCtx<I: NgxHttpModuleImpl>(PhantomData<I>);
pub struct MainCtx<I: NgxHttpModuleImpl>(PhantomData<I>);

impl<I: NgxHttpModuleImpl> CommandCtx for LocCtx<I> {
    type ModuleImpl = NgxHttpModule<I>;
    type Conf = I::LocConf;
    const OFFSET: ngx_uint_t = NGX_RS_HTTP_LOC_CONF_OFFSET;
}
impl<I: NgxHttpModuleImpl> CommandCtx for SrvCtx<I> {
    type ModuleImpl = NgxHttpModule<I>;
    type Conf = I::SrvConf;
    const OFFSET: ngx_uint_t = NGX_RS_HTTP_SRV_CONF_OFFSET;
}
impl<I: NgxHttpModuleImpl> CommandCtx for MainCtx<I> {
    type ModuleImpl = NgxHttpModule<I>;
    type Conf = I::MainConf;
    const OFFSET: ngx_uint_t = NGX_RS_HTTP_MAIN_CONF_OFFSET;
}

pub enum Phase {
    PostRead,
    ServerRewrite,
    FindConfig,
    Rewrite,
    PostRewrite,
    PreAccess,
    Access,
    PostAccess,
    PreContent,
    Content,
    Log,
}
impl From<Phase> for ngx_http_phases {
    fn from(value: Phase) -> Self {
        use Phase::*;
        match value {
            PostRead => ngx_http_phases_NGX_HTTP_POST_READ_PHASE,
            ServerRewrite => ngx_http_phases_NGX_HTTP_SERVER_REWRITE_PHASE,
            FindConfig => ngx_http_phases_NGX_HTTP_FIND_CONFIG_PHASE,
            Rewrite => ngx_http_phases_NGX_HTTP_REWRITE_PHASE,
            PostRewrite => ngx_http_phases_NGX_HTTP_POST_REWRITE_PHASE,
            PreAccess => ngx_http_phases_NGX_HTTP_PREACCESS_PHASE,
            Access => ngx_http_phases_NGX_HTTP_ACCESS_PHASE,
            PostAccess => ngx_http_phases_NGX_HTTP_POST_ACCESS_PHASE,
            PreContent => ngx_http_phases_NGX_HTTP_PRECONTENT_PHASE,
            Content => ngx_http_phases_NGX_HTTP_CONTENT_PHASE,
            Log => ngx_http_phases_NGX_HTTP_LOG_PHASE,
        }
    }
}

pub trait SetHttpHandler {
    fn set_handler<H: HttpHandler>(&mut self) -> Result<(), Status>;
}
impl SetHttpHandler for ngx_conf_t {
    fn set_handler<H: HttpHandler>(&mut self) -> Result<(), Status> {
        let conf = unsafe {
            ngx::http::ngx_http_conf_get_module_main_conf(self, &*addr_of!(ngx_http_core_module))
                .as_mut()
        }
        .ok_or(Status::NGX_ERROR)?;
        let pointer = unsafe {
            (ngx_array_push(&mut conf.phases[ngx_http_phases::from(H::PHASE) as usize].handlers)
                as *mut ngx_http_handler_pt)
                .as_mut()
        }
        .ok_or(Status::NGX_ERROR)?;
        *pointer = Some(handle_func::<H>);
        Ok(())
    }
}

pub trait HttpHandler {
    const PHASE: Phase;
    fn handle(
        request: &mut Request,
        //ctx: &mut <Self::Module as HTTPModule>::Ctx,
    ) -> Status;
}

unsafe extern "C" fn handle_func<H: HttpHandler>(request: *mut ngx_http_request_s) -> ngx_int_t {
    if let Some(request) = request.as_mut() {
        let req = Request::from_ngx_http_request(request);
        return H::handle(req).into();
    }
    Status::NGX_ERROR.into()
}
