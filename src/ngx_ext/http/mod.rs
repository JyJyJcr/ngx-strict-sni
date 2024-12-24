pub mod request;
pub mod variable;

use std::{marker::PhantomData, ptr::addr_of};

use ngx::{
    core::Status,
    ffi::{
        ngx_array_push, ngx_conf_t, ngx_http_core_module, ngx_http_handler_pt, ngx_http_module_t,
        ngx_http_phases, ngx_http_phases_NGX_HTTP_ACCESS_PHASE,
        ngx_http_phases_NGX_HTTP_CONTENT_PHASE, ngx_http_phases_NGX_HTTP_FIND_CONFIG_PHASE,
        ngx_http_phases_NGX_HTTP_LOG_PHASE, ngx_http_phases_NGX_HTTP_POST_ACCESS_PHASE,
        ngx_http_phases_NGX_HTTP_POST_READ_PHASE, ngx_http_phases_NGX_HTTP_POST_REWRITE_PHASE,
        ngx_http_phases_NGX_HTTP_PREACCESS_PHASE, ngx_http_phases_NGX_HTTP_PRECONTENT_PHASE,
        ngx_http_phases_NGX_HTTP_REWRITE_PHASE, ngx_http_phases_NGX_HTTP_SERVER_REWRITE_PHASE,
        ngx_http_request_s, ngx_int_t, ngx_module_t, ngx_uint_t, NGX_RS_HTTP_LOC_CONF_OFFSET,
        NGX_RS_HTTP_MAIN_CONF_OFFSET, NGX_RS_HTTP_SRV_CONF_OFFSET,
    },
    http::{HTTPModule as NgxHTTPModule, Merge, Request},
};

use super::CommandCtx;

pub const fn ngx_http_module_ctx<M: HTTPModule>() -> ngx_http_module_t {
    ngx_http_module_t {
        preconfiguration: Some(HTTPModuleWrapper::<M>::preconfiguration),
        postconfiguration: Some(HTTPModuleWrapper::<M>::postconfiguration),
        create_main_conf: Some(HTTPModuleWrapper::<M>::create_main_conf),
        init_main_conf: Some(HTTPModuleWrapper::<M>::init_main_conf),
        create_srv_conf: Some(HTTPModuleWrapper::<M>::create_srv_conf),
        merge_srv_conf: Some(HTTPModuleWrapper::<M>::merge_srv_conf),
        create_loc_conf: Some(HTTPModuleWrapper::<M>::create_loc_conf),
        merge_loc_conf: Some(HTTPModuleWrapper::<M>::merge_loc_conf),
    }
}

pub struct LocCtx<M: HTTPModule>(PhantomData<M>);
pub struct SrvCtx<M: HTTPModule>(PhantomData<M>);
pub struct MainCtx<M: HTTPModule>(PhantomData<M>);

impl<M: HTTPModule> CommandCtx for LocCtx<M> {
    type Conf = M::LocConf;
    const OFFSET: ngx_uint_t = NGX_RS_HTTP_LOC_CONF_OFFSET;
}
impl<M: HTTPModule> CommandCtx for SrvCtx<M> {
    type Conf = M::SrvConf;
    const OFFSET: ngx_uint_t = NGX_RS_HTTP_SRV_CONF_OFFSET;
}
impl<M: HTTPModule> CommandCtx for MainCtx<M> {
    type Conf = M::MainConf;
    const OFFSET: ngx_uint_t = NGX_RS_HTTP_MAIN_CONF_OFFSET;
}

struct HTTPModuleWrapper<M: HTTPModule>(PhantomData<M>);

impl<M: HTTPModule> NgxHTTPModule for HTTPModuleWrapper<M> {
    type MainConf = <M as HTTPModule>::MainConf;

    type SrvConf = <M as HTTPModule>::SrvConf;

    type LocConf = <M as HTTPModule>::LocConf;

    unsafe extern "C" fn preconfiguration(cf: *mut ngx_conf_t) -> ngx_int_t {
        if let Some(cf) = cf.as_mut() {
            M::preconfiguration(cf).err().unwrap_or(Status::NGX_OK)
        } else {
            Status::NGX_ERROR
        }
        .into()
    }

    unsafe extern "C" fn postconfiguration(cf: *mut ngx_conf_t) -> ngx_int_t {
        if let Some(cf) = cf.as_mut() {
            M::postconfiguration(cf).err().unwrap_or(Status::NGX_OK)
        } else {
            Status::NGX_ERROR
        }
        .into()
    }
}

pub trait HTTPModule {
    fn module_ref() -> &'static ngx_module_t;

    type MainConf: Default + Merge + 'static;
    type SrvConf: Default + Merge + 'static;
    type LocConf: Default + Merge + 'static;
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

pub enum Phase {
    PostRead,
    ServerRewrite,
    FindConfig,
    ReWrite,
    PostRewrite,
    Preaccess,
    Access,
    PostAccess,
    Precontent,
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
            ReWrite => ngx_http_phases_NGX_HTTP_REWRITE_PHASE,
            PostRewrite => ngx_http_phases_NGX_HTTP_POST_REWRITE_PHASE,
            Preaccess => ngx_http_phases_NGX_HTTP_PREACCESS_PHASE,
            Access => ngx_http_phases_NGX_HTTP_ACCESS_PHASE,
            PostAccess => ngx_http_phases_NGX_HTTP_POST_ACCESS_PHASE,
            Precontent => ngx_http_phases_NGX_HTTP_PRECONTENT_PHASE,
            Content => ngx_http_phases_NGX_HTTP_CONTENT_PHASE,
            Log => ngx_http_phases_NGX_HTTP_LOG_PHASE,
        }
    }
}

pub trait SetHTTPHandler {
    fn set_handler<H: HTTPHandler>(&mut self) -> Result<(), Status>;
}
impl SetHTTPHandler for ngx_conf_t {
    fn set_handler<H: HTTPHandler>(&mut self) -> Result<(), Status> {
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

pub trait HTTPHandler {
    type Module: HTTPModule;
    const PHASE: Phase;
    fn handle(
        request: &mut Request,
        //ctx: &mut <Self::Module as HTTPModule>::Ctx,
    ) -> Status;
}

unsafe extern "C" fn handle_func<H: HTTPHandler>(request: *mut ngx_http_request_s) -> ngx_int_t {
    if let Some(request) = request.as_mut() {
        let req = Request::from_ngx_http_request(request);
        return H::handle(req).into();
    }
    Status::NGX_ERROR.into()
}
