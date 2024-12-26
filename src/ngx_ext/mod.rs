pub mod http;
pub mod str;

use bitflags::bitflags;
use core::marker::PhantomData;
use core::{
    ffi::{c_char, c_void},
    ptr::null_mut,
};
use ngx::{
    core::{Status, NGX_CONF_ERROR},
    ffi::{
        nginx_version, ngx_command_t, ngx_conf_t, ngx_cycle_t, ngx_int_t, ngx_log_t, ngx_module_t,
        ngx_str_t, ngx_uint_t, NGX_CONF_TAKE1, NGX_CONF_TAKE2, NGX_HTTP_LOC_CONF,
        NGX_HTTP_MAIN_CONF, NGX_HTTP_SRV_CONF, NGX_RS_MODULE_SIGNATURE,
    },
};

// trait SingleGlobalRoot{
//     type Root;
//     fn root()-> &'static Self::Root;
//     fn root_mut()-> &'static mut Self::Root;
// }
// #[macro_export]
// macro_rules! bind_global_root {
//     ($root:ident:$root_type:ty => $bind_type:ty) => {
//         impl $crate::ngx_ext::SingleGlobalRoot for $bind_type{
//             type = $root_type;
//             fn root() -> &'static Self::Root{
//                 $root
//             }
//             fn root_mut() -> &'static mut Self::Root{
//                 $root
//             }
//         }
//     };
// }

pub trait NgxModule {
    type Impl: NgxModuleImpl<Module = Self>;
    fn module() -> &'static ngx_module_t;
    #[allow(unused_variables)]
    fn init_master(log: &mut ngx_log_t) -> ngx_int_t {
        Status::NGX_OK.into()
    }
    #[allow(unused_variables)]
    fn init_module(cycle: &mut ngx_cycle_t) -> ngx_int_t {
        Status::NGX_OK.into()
    }
    #[allow(unused_variables)]
    fn init_process(cycle: &mut ngx_cycle_t) -> ngx_int_t {
        Status::NGX_OK.into()
    }
    #[allow(unused_variables)]
    fn init_thread(cycle: &mut ngx_cycle_t) -> ngx_int_t {
        Status::NGX_OK.into()
    }
    #[allow(unused_variables)]
    fn exit_thread(cycle: &mut ngx_cycle_t) {}
    #[allow(unused_variables)]
    fn exit_process(cycle: &mut ngx_cycle_t) {}
    #[allow(unused_variables)]
    fn exit_master(cycle: &mut ngx_cycle_t) {}
}
struct NgxModuleCall<M: NgxModule>(PhantomData<M>);
impl<M: NgxModule> NgxModuleCall<M> {
    unsafe extern "C" fn init_master(log: *mut ngx_log_t) -> ngx_int_t {
        M::init_master(&mut *log)
    }
    unsafe extern "C" fn init_module(cycle: *mut ngx_cycle_t) -> ngx_int_t {
        M::init_module(&mut *cycle)
    }
    unsafe extern "C" fn init_process(cycle: *mut ngx_cycle_t) -> ngx_int_t {
        M::init_process(&mut *cycle)
    }
    unsafe extern "C" fn init_thread(cycle: *mut ngx_cycle_t) -> ngx_int_t {
        M::init_thread(&mut *cycle)
    }
    unsafe extern "C" fn exit_thread(cycle: *mut ngx_cycle_t) {
        M::exit_thread(&mut *cycle)
    }
    unsafe extern "C" fn exit_process(cycle: *mut ngx_cycle_t) {
        M::exit_process(&mut *cycle)
    }
    unsafe extern "C" fn exit_master(cycle: *mut ngx_cycle_t) {
        M::exit_master(&mut *cycle)
    }
}

pub const fn ngx_module<M: NgxModule>(
    ctx: &'static mut <M::Impl as NgxModuleImpl>::Ctx,
    command_list_ptr: NgxCommandListPtr<M>,
) -> ngx_module_t {
    ngx_module_t {
        ctx_index: ngx_uint_t::MAX,
        index: ngx_uint_t::MAX,
        name: null_mut(),
        spare0: 0,
        spare1: 0,
        version: nginx_version as ngx_uint_t,
        signature: NGX_RS_MODULE_SIGNATURE.as_ptr() as *const _,

        ctx: ctx as *const _ as *mut _,
        commands: command_list_ptr.raw as *mut _,
        type_: <M::Impl as NgxModuleImpl>::MODULE_TYPE,

        init_master: Some(NgxModuleCall::<M>::init_master),
        init_module: Some(NgxModuleCall::<M>::init_module),
        init_process: Some(NgxModuleCall::<M>::init_process),
        init_thread: Some(NgxModuleCall::<M>::init_thread),
        exit_thread: Some(NgxModuleCall::<M>::exit_thread),
        exit_process: Some(NgxModuleCall::<M>::exit_process),
        exit_master: Some(NgxModuleCall::<M>::exit_master),

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

pub trait NgxModuleImpl {
    type Module: NgxModule;
    const MODULE_TYPE: ngx_uint_t;
    type Ctx: 'static;
}

pub struct NgxCommandListPtr<M: NgxModule> {
    raw: &'static mut ngx_command_t,
    __: PhantomData<M>,
}
impl<M: NgxModule> NgxCommandListPtr<M> {
    pub const fn const_from(value: &'static mut ngx_command_t) -> Self {
        Self {
            raw: value,
            __: PhantomData,
        }
    }
}
// pub struct CommandList<M: NgxModule, const N: usize> {
//     array: [ngx_command_t; N],
// }
// impl<const N: usize> CommandList<N> {
//     pub const fn __new(array: [ngx_command_t; N]) -> Self {
//         Self { array }
//     }
//     pub const fn len(&self) -> usize {
//         N
//     }
// }

pub mod __macro {
    pub use ngx::ffi::ngx_command_t;
    pub use ngx::ngx_null_command;
}

#[macro_export]
macro_rules! command_list {
    (static mut $name:ident : $listname:ident<$module:ty> = [$( $cmd:ty ),*];) => {
        static mut $name:$listname = $listname([
            $($crate::ngx_ext::command::<$module,$cmd>(),)*
            $crate::ngx_ext::__macro::ngx_null_command!()
        ]);


        #[allow(non_camel_case_types)]
        struct $listname([$crate::ngx_ext::__macro::ngx_command_t;{$($crate::one!($cmd) + )+ 1}]);
        #[allow(non_camel_case_types)]
        impl $listname{
            const fn ptr(&'static mut self)->$crate::ngx_ext::NgxCommandListPtr<$module>{
                $crate::ngx_ext::NgxCommandListPtr::const_from(&mut self.0[0])
            }
        }
    };
}

#[macro_export]
macro_rules! one {
    ($cmd:ty) => {
        1usize
    };
}

// trait Command {}

// const fn into_command();

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

pub trait NgxCommand {
    type Ctx: CommandCtx;
    const NAME: ngx_str_t;
    const CONTEXT_FLAG: CommandContextFlag;
    const ARG_FLAG: CommandArgFlag;
    fn handler(cf: &ngx_conf_t, conf: &mut <Self::Ctx as CommandCtx>::Conf) -> Result<(), ()>;
}

pub const fn command<M: NgxModule<Impl = <C::Ctx as CommandCtx>::ModuleImpl>, C: NgxCommand>(
) -> ngx_command_t {
    ngx_command_t {
        name: C::NAME,
        type_: (C::CONTEXT_FLAG.bits() as ngx_uint_t) | (C::ARG_FLAG.bits() as ngx_uint_t),
        set: Some(command_handler::<C>),
        conf: <C::Ctx as CommandCtx>::OFFSET,
        offset: 0,
        post: null_mut(),
    }
}

extern "C" fn command_handler<C: NgxCommand>(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    if let Some(conf) = unsafe { (conf as *mut <C::Ctx as CommandCtx>::Conf).as_mut() } {
        if let Some(cf) = unsafe { cf.as_ref() } {
            if C::handler(cf, conf).is_ok() {
                // NGX_CONF_OK not impled yet, but nullptr = 0 is same as NGX_CONF_OK
                return null_mut();
            }
        }
    }
    NGX_CONF_ERROR as *mut c_char
}

pub trait CommandCtx {
    type ModuleImpl: NgxModuleImpl;
    type Conf;
    const OFFSET: ngx_uint_t;
}
