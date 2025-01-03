//#![cfg_attr(not(test), no_std)]

mod logic;

#[allow(dead_code)]
mod ngx_ext;

mod util;

use core::ffi::CStr;
use core::ptr::addr_of_mut;

use logic::{Analysis, PostReadHandler, PreaccessHandler};
use ngx::ffi::{ngx_conf_t, ngx_str_t};
use ngx::http::{
    ConfCreateError, ConfInitError, ConfigurationDelegate, DefaultMerge, HttpLocConf, HttpMainConf,
    InitConfSetting, Merge, MergeConfigError, NgxHttpModule, NgxHttpModuleCommands,
    NgxHttpModuleCommandsRefMut, SetHttpHandler,
};
use ngx::module::{
    Command, CommandArgFlag, CommandArgFlagSet, CommandContextFlag, CommandContextFlagSet,
    CommandError, NgxModuleCommandsBuilder,
};
use ngx::util::StaticRefMut;
use ngx::{arg_flags, context_flags, ngx_string};
use ngx::{
    exhibit_modules,
    http::{HttpModule, HttpModuleSkel},
};
use ngx_ext::http::variable::{GetHook, VariableHook};

// module exporter
// this macro uses variable name directly.
exhibit_modules!(HttpModuleSkel<StrictSniHttpModule>);

struct StrictSniHttpModule;

impl HttpModule for StrictSniHttpModule {
    const SELF: StaticRefMut<NgxHttpModule<Self>> = {
        static mut MODULE: NgxHttpModule<StrictSniHttpModule> = NgxHttpModule::new();
        unsafe { StaticRefMut::from_mut(&mut *addr_of_mut!(MODULE)) }
    };

    const NAME: &'static CStr = c"strict_sni_module";

    const COMMANDS: NgxHttpModuleCommandsRefMut<Self> = {
        static mut COMMANDS: NgxHttpModuleCommands<StrictSniHttpModule, 3> =
            NgxModuleCommandsBuilder::new()
                .add::<StrictSniCommand>()
                .add::<DirectFilterCommand>()
                .build();
        unsafe { NgxHttpModuleCommandsRefMut::from_mut(&mut *addr_of_mut!(COMMANDS)) }
    };

    type MasterInitializer = ();

    type ModuleDelegate = ();

    type ProcessDelegate = ();

    type ThreadDelegate = ();

    type PreConfiguration = ();

    type PostConfiguration = StrictSniPostConfig;

    type MainConfSetting = StrictSniMainConfManager;
    type SrvConfSetting = DefaultMerge<()>;
    type LocConfSetting = DefaultMerge<ValidationConfig>;
    type Ctx = Analysis;
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

struct StrictSniPostConfig;
impl ConfigurationDelegate for StrictSniPostConfig {
    fn configuration(cf: &mut ngx_conf_t) -> Result<(), ngx::core::Status> {
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

#[derive(Debug)]
struct StrictSniCommon {
    host: VariableHook,
    scheme: VariableHook,
    sni: VariableHook,
}

struct StrictSniMainConfManager;
impl InitConfSetting for StrictSniMainConfManager {
    type Conf = (Option<StrictSniCommon>, ValidationConfig);

    fn create(_: &mut ngx_conf_t) -> Result<Self::Conf, ConfCreateError> {
        Ok(Default::default())
    }

    fn init(cf: &mut ngx_conf_t, (common, _): &mut Self::Conf) -> Result<(), ConfInitError> {
        let vr_host = cf.hook(&ngx_string!("host")).map_err(|_| ConfInitError)?;
        let vr_scheme = cf.hook(&ngx_string!("scheme")).map_err(|_| ConfInitError)?;
        let vr_sni = cf
            .hook(&ngx_string!("ssl_server_name"))
            .map_err(|_| ConfInitError)?;
        *common = Some(StrictSniCommon {
            host: vr_host,
            scheme: vr_scheme,
            sni: vr_sni,
        });
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

impl Merge for ValidationConfig {
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
impl Command for StrictSniCommand {
    type CallRule = HttpLocConf<ValidationConfig>;
    const NAME: ngx_str_t = ngx_string!("strict_sni");

    const CONTEXT_FLAG: CommandContextFlagSet = context_flags!(
        CommandContextFlag::HttpMain,
        CommandContextFlag::HttpSrv,
        CommandContextFlag::HttpLoc
    );

    const ARG_FLAG: CommandArgFlagSet = arg_flags!(CommandArgFlag::Take1);

    fn handler(cf: &mut ngx_conf_t, conf: &mut ValidationConfig) -> Result<(), CommandError> {
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
        Err(CommandError)
    }
}

struct DirectFilterCommand;
impl Command for DirectFilterCommand {
    type CallRule = HttpMainConf<(Option<StrictSniCommon>, ValidationConfig)>;
    const NAME: ngx_str_t = ngx_string!("strict_sni_direct_filter");

    const CONTEXT_FLAG: CommandContextFlagSet = context_flags!(CommandContextFlag::HttpMain);

    const ARG_FLAG: CommandArgFlagSet = arg_flags!(CommandArgFlag::Take1);

    fn handler(
        cf: &mut ngx_conf_t,
        (_, conf): &mut (Option<StrictSniCommon>, ValidationConfig),
    ) -> Result<(), CommandError> {
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
        Err(CommandError)
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

// #[cfg(test)]
// mod test {
//     use core::ptr::null_mut;

//     struct A {
//         x: u32,
//         p: *mut u32,
//     }
//     const fn a() -> A {
//         let mut a = A {
//             x: 1,
//             p: null_mut(),
//         };
//         let p = unsafe { &raw mut AGLBL.x };
//         a.p = p;
//         a
//     }
//     static mut AGLBL: A = a();
//     #[test]
//     fn global_self_pointer_test() {
//         let a = &raw mut AGLBL;
//         let x = unsafe { &mut *a }.x;
//         let p = unsafe { &mut *a }.p;
//         println!("golbal mem {:?} self pointer {:?}", a, p);
//         println!("direct {:} indirect {:}", x, unsafe { *p });
//     }

//     trait TT<X: 'static> {
//         const PTR: &'static mut X;
//     }
//     const fn resolve<X: 'static, T: TT<X>>() -> &'static mut X {
//         T::PTR
//     }
// }
