pub mod request;
pub mod str;
pub mod variable;

use bitflags::bitflags;
use ngx::{
    core::NGX_CONF_ERROR,
    ffi::{
        nginx_version, ngx_command_t, ngx_conf_t, ngx_cycle_t, ngx_http_module_t, ngx_int_t,
        ngx_log_t, ngx_module_t, ngx_str_t, ngx_uint_t, NGX_CONF_TAKE1, NGX_CONF_TAKE2,
        NGX_HTTP_LOC_CONF, NGX_HTTP_MAIN_CONF, NGX_HTTP_MODULE, NGX_HTTP_SRV_CONF,
        NGX_RS_HTTP_LOC_CONF_OFFSET, NGX_RS_HTTP_MAIN_CONF_OFFSET, NGX_RS_HTTP_SRV_CONF_OFFSET,
        NGX_RS_MODULE_SIGNATURE,
    },
    http::HTTPModule,
};
use std::{
    ffi::{c_char, c_void},
    marker::PhantomData,
};

pub enum ModuleType {
    HTTP,
}
impl ModuleType {
    const fn const_into(self) -> ngx_uint_t {
        match self {
            ModuleType::HTTP => NGX_HTTP_MODULE as ngx_uint_t,
        }
    }
}

pub struct CommandList<const N: usize> {
    array: [ngx_command_t; N],
}
impl<const N: usize> CommandList<N> {
    pub const fn __new(array: [ngx_command_t; N]) -> Self {
        Self { array }
    }
}

#[macro_export]
macro_rules! command_list {
    ($name:ident = [$( $cmd:expr ),*];) => {
        const $name:CommandList<{ngx::count!($( $cmd, )+) + 1}> =CommandList::__new([
            $($cmd,)*
            ngx_null_command!()
        ]);
    };
}
// trait Command {}

// const fn into_command();

impl<const N: usize> CommandList<N> {
    const fn array_ptr(&'static self) -> *const ngx_command_t {
        self.array.as_ptr()
    }
}

pub struct NgxModuleBuilder<C: 'static, const N: usize> {
    ctx: &'static C,
    commands_list: &'static CommandList<N>,
    module_type: ModuleType,

    init_master: Option<unsafe extern "C" fn(log: *mut ngx_log_t) -> ngx_int_t>,
    init_module: Option<unsafe extern "C" fn(cycle: *mut ngx_cycle_t) -> ngx_int_t>,
    init_process: Option<unsafe extern "C" fn(cycle: *mut ngx_cycle_t) -> ngx_int_t>,
    init_thread: Option<unsafe extern "C" fn(cycle: *mut ngx_cycle_t) -> ngx_int_t>,
    exit_thread: Option<unsafe extern "C" fn(cycle: *mut ngx_cycle_t)>,
    exit_process: Option<unsafe extern "C" fn(cycle: *mut ngx_cycle_t)>,
    exit_master: Option<unsafe extern "C" fn(cycle: *mut ngx_cycle_t)>,
}
impl<C: 'static, const N: usize> NgxModuleBuilder<C, N> {
    pub const fn new(
        ctx: &'static C,
        commands_list: &'static CommandList<N>,
        module_type: ModuleType,
    ) -> Self {
        Self {
            ctx,
            commands_list,
            module_type,

            init_master: None,
            init_module: None,
            init_process: None,
            init_thread: None,
            exit_thread: None,
            exit_process: None,
            exit_master: None,
        }
    }
    pub const fn build(self) -> ngx_module_t {
        ngx_module_t {
            ctx_index: ngx_uint_t::max_value(),
            index: ngx_uint_t::max_value(),
            name: std::ptr::null_mut(),
            spare0: 0,
            spare1: 0,
            version: nginx_version as ngx_uint_t,
            signature: NGX_RS_MODULE_SIGNATURE.as_ptr() as *const _,

            ctx: self.ctx as *const _ as *mut _,
            commands: self.commands_list.array_ptr() as *mut _,
            type_: self.module_type.const_into(),

            init_master: self.init_master,
            init_module: self.init_module,
            init_process: self.init_process,
            init_thread: self.init_thread,
            exit_thread: self.exit_thread,
            exit_process: self.exit_process,
            exit_master: self.exit_master,

            spare_hook0: 0,
            spare_hook1: 0,
            spare_hook2: 0,
            spare_hook3: 0,
            spare_hook4: 0,
            spare_hook5: 0,
            spare_hook6: 0,
            spare_hook7: 0,
        }
    }
    #[allow(dead_code)]
    pub const fn init_master(
        mut self,
        f: unsafe extern "C" fn(log: *mut ngx_log_t) -> ngx_int_t,
    ) -> Self {
        self.init_master = Some(f);
        self
    }
    #[allow(dead_code)]
    pub const fn init_module(
        mut self,
        f: unsafe extern "C" fn(cycle: *mut ngx_cycle_t) -> ngx_int_t,
    ) -> Self {
        self.init_module = Some(f);
        self
    }
    #[allow(dead_code)]
    pub const fn init_process(
        mut self,
        f: unsafe extern "C" fn(cycle: *mut ngx_cycle_t) -> ngx_int_t,
    ) -> Self {
        self.init_process = Some(f);
        self
    }
    #[allow(dead_code)]
    pub const fn init_thread(
        mut self,
        f: unsafe extern "C" fn(cycle: *mut ngx_cycle_t) -> ngx_int_t,
    ) -> Self {
        self.init_thread = Some(f);
        self
    }
    #[allow(dead_code)]
    pub const fn exit_thread(mut self, f: unsafe extern "C" fn(cycle: *mut ngx_cycle_t)) -> Self {
        self.exit_thread = Some(f);
        self
    }
    #[allow(dead_code)]
    pub const fn exit_process(mut self, f: unsafe extern "C" fn(cycle: *mut ngx_cycle_t)) -> Self {
        self.exit_process = Some(f);
        self
    }
    #[allow(dead_code)]
    pub const fn exit_master(mut self, f: unsafe extern "C" fn(cycle: *mut ngx_cycle_t)) -> Self {
        self.exit_master = Some(f);
        self
    }
}

pub const fn ngx_http_module_ctx<M: HTTPModule>() -> ngx_http_module_t {
    ngx_http_module_t {
        preconfiguration: Some(M::preconfiguration),
        postconfiguration: Some(M::postconfiguration),
        create_main_conf: Some(M::create_main_conf),
        init_main_conf: Some(M::init_main_conf),
        create_srv_conf: Some(M::create_srv_conf),
        merge_srv_conf: Some(M::merge_srv_conf),
        create_loc_conf: Some(M::create_loc_conf),
        merge_loc_conf: Some(M::merge_loc_conf),
    }
}

bitflags! {
    pub struct CommandContextFlag:u32 {
        const Main = NGX_HTTP_MAIN_CONF;
        const Srv = NGX_HTTP_SRV_CONF;
        const Loc = NGX_HTTP_LOC_CONF;
    }
    pub struct CommandArgFlag:u32 {
        const Take1 = NGX_CONF_TAKE1;
        const Take2 = NGX_CONF_TAKE2;
    }
}

pub trait Command {
    type Ctx: CommandCtx;
    const NAME: ngx_str_t;
    const CONTEXT_FLAG: CommandContextFlag;
    const ARG_FLAG: CommandArgFlag;
    fn handler(cf: &ngx_conf_t, conf: &mut <Self::Ctx as CommandCtx>::Conf) -> Result<(), ()>;
}

pub const fn command<C: Command>() -> ngx_command_t {
    ngx_command_t {
        name: C::NAME,
        type_: (C::CONTEXT_FLAG.bits() as ngx_uint_t) | (C::ARG_FLAG.bits() as ngx_uint_t),
        set: Some(command_handler::<C>),
        conf: <C::Ctx as CommandCtx>::OFFSET,
        offset: 0,
        post: std::ptr::null_mut(),
    }
}

extern "C" fn command_handler<C: Command>(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    if let Some(conf) = unsafe { (conf as *mut <C::Ctx as CommandCtx>::Conf).as_mut() } {
        if let Some(cf) = unsafe { cf.as_ref() } {
            if C::handler(cf, conf).is_ok() {
                // NGX_CONF_OK not impled yet, but nullptr = 0 is same as NGX_CONF_OK
                return std::ptr::null_mut();
            }
        }
    }
    NGX_CONF_ERROR as *mut c_char
}

pub trait CommandCtx {
    type Conf;
    const OFFSET: ngx_uint_t;
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
