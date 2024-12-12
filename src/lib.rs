use ngx::ffi::{
    nginx_version, ngx_array_push, ngx_command_t, ngx_conf_t, ngx_http_core_module,
    ngx_http_handler_pt, ngx_http_module_t, ngx_http_phases_NGX_HTTP_ACCESS_PHASE,
    ngx_http_request_t, ngx_inet_get_port, ngx_int_t, ngx_module_t, ngx_ssl_get_server_name,
    ngx_str_t, ngx_uint_t, NGX_CONF_TAKE1, NGX_HTTP_LOC_CONF, NGX_HTTP_MAIN_CONF, NGX_HTTP_MODULE,
    NGX_HTTP_SRV_CONF, NGX_RS_HTTP_LOC_CONF_OFFSET, NGX_RS_MODULE_SIGNATURE,
};
use ngx::http::MergeConfigError;
use ngx::{core, core::Status, http, http::HTTPModule};
use ngx::{
    http_request_handler, ngx_log_debug_http, ngx_modules, ngx_null_command, ngx_null_string,
    ngx_string,
};
use std::os::raw::{c_char, c_void};
use std::ptr::addr_of;

ngx_modules!(strict_sni_module);

#[no_mangle]
#[allow(non_upper_case_globals)]
pub static mut strict_sni_module: ngx_module_t = ngx_module_t {
    ctx_index: ngx_uint_t::max_value(),
    index: ngx_uint_t::max_value(),
    name: std::ptr::null_mut(),
    spare0: 0,
    spare1: 0,
    version: nginx_version as ngx_uint_t,
    signature: NGX_RS_MODULE_SIGNATURE.as_ptr() as *const c_char,

    ctx: &strict_sni_module_ctx as *const _ as *mut _,
    commands: unsafe { &strict_sni_commands[0] as *const _ as *mut _ },
    type_: NGX_HTTP_MODULE as ngx_uint_t,

    init_master: None,
    init_module: None,
    init_process: None,
    init_thread: None,
    exit_thread: None,
    exit_process: None,
    exit_master: None,

    spare_hook0: 0,
    spare_hook1: 0,
    spare_hook2: 0,
    spare_hook3: 0,
    spare_hook4: 0,
    spare_hook5: 0,
    spare_hook6: 0,
    spare_hook7: 0,
};

#[no_mangle]
#[allow(non_upper_case_globals)]
static strict_sni_module_ctx: ngx_http_module_t = ngx_http_module_t {
    preconfiguration: Some(Module::preconfiguration),
    postconfiguration: Some(Module::postconfiguration),
    create_main_conf: Some(Module::create_main_conf),
    init_main_conf: Some(Module::init_main_conf),
    create_srv_conf: Some(Module::create_srv_conf),
    merge_srv_conf: Some(Module::merge_srv_conf),
    create_loc_conf: Some(Module::create_loc_conf),
    merge_loc_conf: Some(Module::merge_loc_conf),
};

struct Module;

impl http::HTTPModule for Module {
    type MainConf = ();
    type SrvConf = ();
    type LocConf = ModuleConfig;
    unsafe extern "C" fn postconfiguration(cf: *mut ngx_conf_t) -> ngx_int_t {
        match unsafe {
            http::ngx_http_conf_get_module_main_conf(cf, &*addr_of!(ngx_http_core_module)).as_mut()
        } {
            Some(conf) => {
                match unsafe {
                    (ngx_array_push(
                        &mut conf.phases[ngx_http_phases_NGX_HTTP_ACCESS_PHASE as usize].handlers,
                    ) as *mut ngx_http_handler_pt)
                        .as_mut()
                } {
                    Some(pointer) => {
                        *pointer = Some(strict_sni_access_handler);
                        return Status::NGX_OK.into();
                    }
                    None => {}
                }
            }
            None => {}
        }
        Status::NGX_ERROR.into()
    }
}

#[derive(Debug, Default)]
struct ModuleConfig {
    status: ModuleStatus,
}
#[derive(Debug, Default, Clone)]
enum ModuleStatus {
    #[default]
    UNSET,
    OFF,
    ON,
}

impl http::Merge for ModuleConfig {
    fn merge(&mut self, prev: &ModuleConfig) -> Result<(), MergeConfigError> {
        match self.status {
            ModuleStatus::UNSET => {
                self.status = prev.status.clone();
            }
            _ => {}
        };
        Ok(())
    }
}

#[no_mangle]
#[allow(non_upper_case_globals)]
static mut strict_sni_commands: [ngx_command_t; 2] = [
    ngx_command_t {
        name: ngx_string!("strict_sni"),
        type_: (NGX_HTTP_MAIN_CONF | NGX_HTTP_SRV_CONF | NGX_HTTP_LOC_CONF | NGX_CONF_TAKE1)
            as ngx_uint_t,
        set: Some(strict_sni_command_handler),
        conf: NGX_RS_HTTP_LOC_CONF_OFFSET,
        offset: 0,
        post: std::ptr::null_mut(),
    },
    ngx_null_command!(),
];

#[no_mangle]
extern "C" fn strict_sni_command_handler(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    match unsafe { (conf as *mut ModuleConfig).as_mut() } {
        Some(conf) => match unsafe { cf.as_ref() } {
            Some(cf) => match unsafe { cf.args.as_ref() } {
                Some(args) => {
                    match unsafe { (args.elts as *mut ngx_str_t).add(1).as_ref() } {
                        Some(ngx_arg) => {
                            let arg = ngx_arg.to_str();
                            use ModuleStatus::*;
                            if arg.eq_ignore_ascii_case("on") {
                                conf.status = ON;
                            } else if arg.eq_ignore_ascii_case("off") {
                                conf.status = OFF;
                            }
                        }
                        None => {}
                    };
                }
                None => {}
            },
            None => {}
        },
        None => {}
    }
    std::ptr::null_mut()
}

http_request_handler!(strict_sni_access_handler, |request: &mut http::Request| {
    let co = unsafe { request.get_module_loc_conf::<ModuleConfig>(&*addr_of!(strict_sni_module)) };
    let co = co.expect("module config is none");

    // ngx_log_debug_http!(request, "strict_sni module enabled: {:?}", co.status);

    match co.status {
        ModuleStatus::ON => match unsafe { request.connection().as_mut() } {
            None => {
                // ngx_log_debug_http!(request, "strict_sni connection() nullptr");
            }
            Some(con) => {
                if con.ssl.is_null() {
                    // ngx_log_debug_http!(request, "strict_sni ssl nullptr");
                } else {
                    let mut name = ngx_null_string!();
                    if unsafe { ngx_ssl_get_server_name(con, request.get_inner().pool, &mut name) }
                        != Status::NGX_OK.into()
                    {
                        // ngx_log_debug_http!(request, "strict_sni ssl_servername nullptr");
                    } else {
                        let sni = name.to_str();
                        let port = if con.local_sockaddr.is_null() {
                            None
                        } else {
                            Some(unsafe { ngx_inet_get_port(con.local_sockaddr) })
                        };
                        // ngx_log_debug_http!(request, "strict_sni ssl_servername \"{}\"", sni);
                        for (k, v) in request.headers_in_iterator() {
                            if k.eq_ignore_ascii_case("host") {
                                match port {
                                    Some(port) => {
                                        if !(v.eq_ignore_ascii_case(
                                            format!("{}:{}", sni, port).as_str(),
                                        ) || (port == 443 && v.eq_ignore_ascii_case(sni)))
                                        {
                                            ngx_log_debug_http!(
                                                request,
                                                "strict_sni violation: ssl_servername: \"{}\", port: \"{}\" != host: \"{}\"",
                                                sni,
                                                port,
                                                v
                                            );
                                            return http::HTTPStatus::MISDIRECTED_REQUEST.into();
                                        } else {
                                            // ngx_log_debug_http!(
                                            //     request,
                                            //     "strict_sni pass: ssl_servername: \"{}\", port: \"{}\" == host: \"{}\"",
                                            //     sni,
                                            //     port,
                                            //     v
                                            // );
                                        }
                                    }
                                    None => {
                                        if !v.eq_ignore_ascii_case(sni) {
                                            ngx_log_debug_http!(
                                            request,
                                            "strict_sni violation: ssl_servername: \"{}\" != host: \"{}\"",
                                            sni,
                                            v
                                        );
                                            return http::HTTPStatus::MISDIRECTED_REQUEST.into();
                                        } else {
                                            // ngx_log_debug_http!(
                                            //     request,
                                            //     "strict_sni pass: ssl_servername: \"{}\" == host: \"{}\"",
                                            //     sni,
                                            //     v
                                            // );
                                        }
                                    }
                                };
                            }
                        }
                    }
                }
            }
        },
        _ => {}
    }
    core::Status::NGX_DECLINED
});
