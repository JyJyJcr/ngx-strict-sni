//#![no_std]

mod logic;

#[allow(dead_code)]
mod ngx_ext;

mod util;

use core::ptr::addr_of;
use core::ptr::addr_of_mut;
use logic::{Analysis, PostReadHandler, PreaccessHandler};
use ngx::ffi::{ngx_conf_t, ngx_http_module_t, ngx_module_t, ngx_str_t};
use ngx::http::MergeConfigError;
use ngx::{core::Status, http};
use ngx::{ngx_modules, ngx_string};
use ngx_ext::http::variable::GetHook;
use ngx_ext::http::{ngx_http_module_ctx, variable::VariableHook, LocCtx, MainCtx};
use ngx_ext::http::{
    ConfInitManager, DefaultConfManager, NgxHttpModule, NgxHttpModuleImpl, SetHttpHandler,
};
use ngx_ext::{ngx_module, CommandArgFlag, CommandContextFlag, NgxCommand, NgxModule};

// module exporter
// this macro uses variable name directly.
ngx_modules!(strict_sni_module);

// so here we surpress non_upper_case warning.
#[allow(non_upper_case_globals)]
static mut strict_sni_module: ngx_module_t = ngx_module::<StrictSniModule>(
    unsafe { &mut *addr_of_mut!(STRICT_SNI_MODULE_CTX) },
    unsafe { (&mut *addr_of_mut!(STRICT_SNI_COMMAND_LIST)).ptr() },
);

struct StrictSniModule;
impl NgxModule for StrictSniModule {
    type Impl = NgxHttpModule<StrictSniHttpModuleImpl>;

    fn module() -> &'static ngx_module_t {
        unsafe { &*addr_of!(strict_sni_module) }
    }
    // fn init_module(cycle: &mut ngx_cycle_t) -> ngx_int_t {
    //     ngx_log_debug!(cycle.log, "strict_sni module init_master called");
    //     if !cycle.modules.is_null() {
    //         let mos_p = slice_from_raw_parts(cycle.modules, cycle.modules_n);
    //         if let Some(mos) = unsafe { mos_p.as_ref() } {
    //             for &mop in mos {
    //                 if let Some(module) = unsafe { mop.as_ref() } {
    //                     let namep: *const c_char = module.name;
    //                     if namep.is_null() {
    //                         ngx_log_debug!(cycle.log, "strict_sni [module list]: name nullptr");
    //                     } else {
    //                         let name = unsafe { CStr::from_ptr(namep) };
    //                         if let Ok(name) = name.to_str() {
    //                             ngx_log_debug!(cycle.log, "strict_sni [module list]: \"{}\"", name);
    //                         } else {
    //                             ngx_log_debug!(cycle.log, "strict_sni [module list]: name invalid");
    //                         }
    //                     }
    //                 }
    //             }
    //         }
    //         Status::NGX_OK.into()
    //     } else {
    //         Status::NGX_ERROR.into()
    //     }
    // }
}

command_list!(
    static mut STRICT_SNI_COMMAND_LIST: CommandList<StrictSniModule> =
        [StrictSniCommand, DirectFilterCommand];
);

static mut STRICT_SNI_MODULE_CTX: ngx_http_module_t =
    ngx_http_module_ctx::<StrictSniHttpModuleImpl>();

#[derive(Debug)]
struct StrictSniCommon {
    host: VariableHook,
    scheme: VariableHook,
    sni: VariableHook,
}

//static MODULE_DATA: OnceLock<StrictSniCommon> = OnceLock::new();

struct StrictSniMainConfManager;
impl ConfInitManager for StrictSniMainConfManager {
    type Conf = (Option<StrictSniCommon>, ValidationConfig);

    fn create(_: &mut ngx_conf_t) -> Result<Self::Conf, ()> {
        Ok(Default::default())
    }

    fn init(cf: &mut ngx_conf_t, (common, _): &mut Self::Conf) -> Result<(), ()> {
        let vr_host = cf.hook(&ngx_string!("host")).map_err(|_| ())?;
        let vr_scheme = cf.hook(&ngx_string!("scheme")).map_err(|_| ())?;
        let vr_sni = cf.hook(&ngx_string!("ssl_server_name")).map_err(|_| ())?;
        *common = Some(StrictSniCommon {
            host: vr_host,
            scheme: vr_scheme,
            sni: vr_sni,
        });
        Ok(())
    }
}

struct StrictSniHttpModuleImpl;
impl NgxHttpModuleImpl for StrictSniHttpModuleImpl {
    type Module = StrictSniModule;
    type MainConf = (Option<StrictSniCommon>, ValidationConfig);
    type SrvConf = ();
    type LocConf = ValidationConfig;
    type MainConfManager = StrictSniMainConfManager;
    type SrvConfManager = DefaultConfManager<()>;
    type LocConfManager = DefaultConfManager<Self::LocConf>;
    type Ctx = Analysis;
    fn postconfiguration(cf: &mut ngx_conf_t) -> Result<(), Status> {
        // if let Some(log) = unsafe { cf.log.as_mut() } {
        //     ngx_log_debug!(log, "strict_sni check pool");
        //     if let Some(cy) = unsafe { cf.cycle.as_ref() } {
        //         ngx_log_debug!(log, "strict_sni check pool same: {}", cy.pool == cf.pool);
        //         if cy.pool == cf.pool {
        //             return Err(Status::NGX_ERROR);
        //         }
        //     }
        // }
        // if let Some(a)=unsafe{cf.cycle.as_mut()}{
        //     cf.pool
        // }
        // let pool=Pool::from_ngx_pool(pool)
        // ngx_

        cf.set_handler::<PostReadHandler>()?;
        cf.set_handler::<PreaccessHandler>()?;

        // let pool = unsafe { cf.pool.as_mut() }.ok_or(Status::NGX_ERROR)?;
        // let mut pool = unsafe { Pool::from_ngx_pool(pool) };
        // pool.allocate(common);
        Ok(())
    }
}

#[derive(Debug, Default)]
struct ValidationConfig {
    rfc_mode: CheckSwitch<()>,
    port_mode: CheckSwitch<()>,
    host_mode: CheckSwitch<HostCheckRigor>,
}

// impl Drop for ModuleConfig {
//     fn drop(&mut self) {
//         todo!()
//     }
// }

#[derive(Debug, Default, Clone)]
enum CheckSwitch<M> {
    #[default]
    Unset,
    Off,
    On(M),
}

#[derive(Debug, Clone)]
enum HostCheckRigor {
    Normal,
    Strict,
}

// #[derive(Debug, Clone)]
// enum RfcChecRigor {

// }

impl http::Merge for ValidationConfig {
    fn merge(&mut self, prev: &ValidationConfig) -> Result<(), MergeConfigError> {
        if let CheckSwitch::Unset = self.rfc_mode {
            self.rfc_mode = prev.rfc_mode.clone();
        };
        if let CheckSwitch::Unset = self.port_mode {
            self.port_mode = prev.port_mode.clone();
        };
        if let CheckSwitch::Unset = self.host_mode {
            self.host_mode = prev.host_mode.clone();
        };
        Ok(())
    }
}

struct StrictSniCommand;
impl NgxCommand for StrictSniCommand {
    type Ctx = LocCtx<StrictSniHttpModuleImpl>;
    const NAME: ngx_str_t = ngx_string!("strict_sni");

    const CONTEXT_FLAG: ngx_ext::CommandContextFlag = {
        CommandContextFlag::Main
            .union(CommandContextFlag::Srv)
            .union(CommandContextFlag::Loc)
    };

    const ARG_FLAG: ngx_ext::CommandArgFlag = CommandArgFlag::Take1;

    fn handler(cf: &ngx_conf_t, conf: &mut ValidationConfig) -> Result<(), ()> {
        if let Some(args) = unsafe { cf.args.as_ref() } {
            if let Some(ngx_arg) = unsafe { (args.elts as *mut ngx_str_t).add(1).as_ref() } {
                let arg = ngx_arg.to_str();
                // good old on/off
                if arg.eq_ignore_ascii_case("on") {
                    conf.port_mode = CheckSwitch::On(());
                    conf.host_mode = CheckSwitch::On(HostCheckRigor::Normal);
                } else if arg.eq_ignore_ascii_case("off") {
                    conf.port_mode = CheckSwitch::Off;
                    conf.host_mode = CheckSwitch::Off;
                } else if arg.eq_ignore_ascii_case("strict") {
                    conf.port_mode = CheckSwitch::On(());
                    conf.host_mode = CheckSwitch::On(HostCheckRigor::Strict);
                }
                // port only setting
                if arg.eq_ignore_ascii_case("port") {
                    conf.port_mode = CheckSwitch::On(());
                } else if arg.eq_ignore_ascii_case("no_port") {
                    conf.port_mode = CheckSwitch::Off;
                }
                // host only setting
                if arg.eq_ignore_ascii_case("host") {
                    conf.host_mode = CheckSwitch::On(HostCheckRigor::Normal);
                } else if arg.eq_ignore_ascii_case("strict_host") {
                    conf.host_mode = CheckSwitch::On(HostCheckRigor::Strict);
                } else if arg.eq_ignore_ascii_case("no_host") {
                    conf.host_mode = CheckSwitch::Off;
                }
                return Ok(());
            };
        }
        Err(())
    }
}

struct DirectFilterCommand;
impl NgxCommand for DirectFilterCommand {
    type Ctx = MainCtx<StrictSniHttpModuleImpl>;
    const NAME: ngx_str_t = ngx_string!("strict_sni_direct_filter");

    const CONTEXT_FLAG: ngx_ext::CommandContextFlag = { CommandContextFlag::Main };

    const ARG_FLAG: ngx_ext::CommandArgFlag = CommandArgFlag::Take1;

    fn handler(
        cf: &ngx_conf_t,
        (_, conf): &mut (Option<StrictSniCommon>, ValidationConfig),
    ) -> Result<(), ()> {
        if let Some(args) = unsafe { cf.args.as_ref() } {
            if let Some(ngx_arg) = unsafe { (args.elts as *mut ngx_str_t).add(1).as_ref() } {
                let arg = ngx_arg.to_str();
                // good old on/off
                if arg.eq_ignore_ascii_case("on") {
                    conf.port_mode = CheckSwitch::On(());
                    conf.host_mode = CheckSwitch::On(HostCheckRigor::Normal);
                } else if arg.eq_ignore_ascii_case("off") {
                    conf.port_mode = CheckSwitch::Off;
                    conf.host_mode = CheckSwitch::Off;
                } else if arg.eq_ignore_ascii_case("strict") {
                    conf.port_mode = CheckSwitch::On(());
                    conf.host_mode = CheckSwitch::On(HostCheckRigor::Strict);
                }
                // port only setting
                if arg.eq_ignore_ascii_case("port") {
                    conf.port_mode = CheckSwitch::On(());
                } else if arg.eq_ignore_ascii_case("no_port") {
                    conf.port_mode = CheckSwitch::Off;
                }
                // host only setting
                if arg.eq_ignore_ascii_case("host") {
                    conf.host_mode = CheckSwitch::On(HostCheckRigor::Normal);
                } else if arg.eq_ignore_ascii_case("strict_host") {
                    conf.host_mode = CheckSwitch::On(HostCheckRigor::Strict);
                } else if arg.eq_ignore_ascii_case("no_host") {
                    conf.host_mode = CheckSwitch::Off;
                }
                return Ok(());
            };
        }
        Err(())
    }
}

// #[allow(non_upper_case_globals)]
// static mut client_certificate_filter_module: ngx_module_t =
//     ngx_module::<ClientCertificateFilterModule>(
//         unsafe { &mut *addr_of_mut!(STRICT_SNI_MODULE_CTX) },
//         unsafe { (&mut *addr_of_mut!(STRICT_SNI_COMMAND_LIST)).ptr() },
//     );
// struct ClientCertificateFilterModule;
// impl NgxModule for ClientCertificateFilterModule {
//     type Impl = NgxHttpModule<StrictSniHttpModuleImpl>;

//     fn module() -> &'static ngx_module_t {
//         unsafe { &*addr_of!(client_certificate_filter_module) }
//     }
// }
