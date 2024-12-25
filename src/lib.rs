mod logic;

#[allow(dead_code)]
mod ngx_ext;

mod util;

use logic::{Analysis, PostReadHandler, PreaccessHandler};
use ngx::ffi::{ngx_conf_t, ngx_http_module_t, ngx_module_t, ngx_str_t};
use ngx::http::MergeConfigError;
use ngx::{core::Status, http};
use ngx::{ngx_modules, ngx_string};
use ngx_ext::http::variable::GetHook;
use ngx_ext::http::SetHTTPHandler;
use ngx_ext::http::{ngx_http_module_ctx, variable::VariableHook, HTTPModule, LocCtx, MainCtx};
use ngx_ext::{
    command, Command, CommandArgFlag, CommandContextFlag, CommandList, ModuleType, NgxModuleBuilder,
};
use std::ptr::addr_of;
use std::sync::OnceLock;

// this macro uses variable name directly.
ngx_modules!(strict_sni_module);

// so her we surpress non_upper_case warning.
#[allow(non_upper_case_globals)]
static mut strict_sni_module: ngx_module_t = NgxModuleBuilder::new(
    &STRICT_SNI_MODULE_CTX,
    &STRICT_SNI_COMMAND_LIST,
    ModuleType::Http,
)
//.init_module(init_module)
.build();

// unsafe extern "C" fn init_module(cycle: *mut ngx_cycle_t) -> ngx_int_t {
//     if let Some(cycle) = cycle.as_ref() {
//         ngx_log_debug!(cycle.log, "strict_sni module init_master called");
//     }
//     Status::NGX_OK.into()
// }

// const fn module_ref() -> &'static ngx_module_t {
//     unsafe { &*&raw const strict_sni_module }
// }

command_list!(
    STRICT_SNI_COMMAND_LIST = [command::<StrictSniCommand>(),command::<DirectFilterCommand>()];
);

const STRICT_SNI_MODULE_CTX: ngx_http_module_t = ngx_http_module_ctx::<StrictSniHttpModule>();

struct StrictSniHttpModule;

#[derive(Debug)]
struct StrictSniCommon {
    host: VariableHook,
    scheme: VariableHook,
    sni: VariableHook,
}

static MODULE_DATA: OnceLock<StrictSniCommon> = OnceLock::new();

impl HTTPModule for StrictSniHttpModule {
    fn module_ref() -> &'static ngx_module_t {
        unsafe { &*addr_of!(strict_sni_module) }
    }
    type MainConf = ValidationConfig;
    type SrvConf = ();
    type LocConf = ValidationConfig;
    type Ctx = Analysis;
    fn postconfiguration(cf: &mut ngx_conf_t) -> Result<(), Status> {
        cf.set_handler::<PostReadHandler>()?;
        cf.set_handler::<PreaccessHandler>()?;

        let vr_host = cf
            .hook(&ngx_string!("host"))
            .map_err(|_| Status::NGX_ERROR)?;
        let vr_scheme = cf
            .hook(&ngx_string!("scheme"))
            .map_err(|_| Status::NGX_ERROR)?;
        let vr_sni = cf
            .hook(&ngx_string!("ssl_server_name"))
            .map_err(|_| Status::NGX_ERROR)?;
        let _ = MODULE_DATA.set(StrictSniCommon {
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
impl Command for StrictSniCommand {
    type Ctx = LocCtx<StrictSniHttpModule>;
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
impl Command for DirectFilterCommand {
    type Ctx = MainCtx<StrictSniHttpModule>;
    const NAME: ngx_str_t = ngx_string!("strict_sni_direct_filter");

    const CONTEXT_FLAG: ngx_ext::CommandContextFlag = { CommandContextFlag::Main };

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
