mod ngx_ext;
mod util;

use ::core::str;
use fluent_uri::{Uri, UriRef};
use ngx::ffi::{
    nginx_version, ngx_array_push, ngx_command_t, ngx_conf_s, ngx_conf_t, ngx_connection_t,
    ngx_cycle_t, ngx_http_core_module, ngx_http_get_flushed_variable, ngx_http_get_variable,
    ngx_http_get_variable_index, ngx_http_get_variable_pt, ngx_http_handler_pt, ngx_http_module_t,
    ngx_http_phases_NGX_HTTP_ACCESS_PHASE, ngx_http_phases_NGX_HTTP_POST_READ_PHASE,
    ngx_http_phases_NGX_HTTP_PREACCESS_PHASE, ngx_http_request_t, ngx_http_ssl_certificate,
    ngx_inet_get_port, ngx_int_t, ngx_log_t, ngx_module_t, ngx_ssl_certificate_index,
    ngx_ssl_get_server_name, ngx_ssl_get_subject_dn, ngx_str_t, ngx_uint_t, EVP_sha1,
    SSL_get0_peer_certificate, SSL_get_certificate, X509_LOOKUP_by_fingerprint, X509_check_host,
    X509_digest, X509_free, X509_get_serialNumber, EVP_MAX_MD_SIZE, NGX_CONF_TAKE1, NGX_ERROR,
    NGX_HTTP_LOC_CONF, NGX_HTTP_MAIN_CONF, NGX_HTTP_MODULE, NGX_HTTP_SRV_CONF, NGX_HTTP_VERSION_20,
    NGX_OK, NGX_RS_HTTP_LOC_CONF_OFFSET, NGX_RS_MODULE_SIGNATURE, X509,
};
use ngx::http::{HTTPStatus, MergeConfigError, Request};
use ngx::{core, core::Status, http};
use ngx::{
    http_request_handler, ngx_log_debug, ngx_log_debug_http, ngx_modules, ngx_null_command,
    ngx_null_string, ngx_string,
};
use ngx_ext::http::variable::GetHook;
use ngx_ext::http::{
    ngx_http_module_ctx, request::RequestExt, variable::VariableHook, HTTPModule, LocCtx, MainCtx,
};
use ngx_ext::http::{HTTPHandler, Phase, SetHTTPHandler};
use ngx_ext::{
    command, Command, CommandArgFlag, CommandContextFlag, CommandList, ModuleType, NgxModuleBuilder,
};
use std::cell::OnceCell;
use std::num::ParseIntError;
use std::os::raw::{c_char, c_void};
use std::ptr::{addr_of, addr_of_mut, slice_from_raw_parts, slice_from_raw_parts_mut};
use std::slice::from_raw_parts;
use std::str::{from_utf8, from_utf8_unchecked};
use std::sync::{LazyLock, OnceLock};
use std::{default, slice};
use util::{parse_host_header, parse_request_line};

// this macro uses variable name directly.
ngx_modules!(strict_sni_module);

// so her we surpress non_upper_case warning.
#[allow(non_upper_case_globals)]
static mut strict_sni_module: ngx_module_t = NgxModuleBuilder::new(
    &STRICT_SNI_MODULE_CTX,
    &STRICT_SNI_COMMAND_LIST,
    ModuleType::HTTP,
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
            .hook(&mut ngx_string!("host"))
            .map_err(|_| Status::NGX_ERROR)?;
        let vr_scheme = cf
            .hook(&mut ngx_string!("scheme"))
            .map_err(|_| Status::NGX_ERROR)?;
        let vr_sni = cf
            .hook(&mut ngx_string!("ssl_server_name"))
            .map_err(|_| Status::NGX_ERROR)?;
        let _ = MODULE_DATA.set(StrictSniCommon {
            host: vr_host,
            scheme: vr_scheme,
            sni: vr_sni,
        });
        Ok(())
    }
}

// if let Some(conf) =
//             http::ngx_http_conf_get_module_main_conf(cf, &*addr_of!(ngx_http_core_module)).as_mut()
//         {
//             if let Some(pointer) = unsafe {
//                 (ngx_array_push(
//                     &mut conf.phases[ngx_http_phases_NGX_HTTP_POST_READ_PHASE as usize].handlers,
//                 ) as *mut ngx_http_handler_pt)
//                     .as_mut()
//             } {
//                 *pointer = Some(post_read_handler);
//                 if let Some(pointer) = unsafe {
//                     (ngx_array_push(
//                         &mut conf.phases[ngx_http_phases_NGX_HTTP_PREACCESS_PHASE as usize]
//                             .handlers,
//                     ) as *mut ngx_http_handler_pt)
//                         .as_mut()
//                 } {
//                     *pointer = Some(preaccess_handler);

#[derive(Debug, Default)]
struct ValidationConfig {
    //rfc_mode: RfcCheckMode,
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

impl http::Merge for ValidationConfig {
    fn merge(&mut self, prev: &ValidationConfig) -> Result<(), MergeConfigError> {
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

struct PostReadHandler;
impl HTTPHandler for PostReadHandler {
    type Module = StrictSniHttpModule;

    const PHASE: Phase = Phase::PostRead;

    fn handle(request: &mut Request) -> Status {
        ngx_log_debug_http!(request, "strict_sni post_read_handler called");
        let aner: Analyzer = MODULE_DATA.get().unwrap().into();
        if let Ok(analysis) = aner.analyze(request) {
            ngx_log_debug_http!(request, "strict_sni analyze succ");
            let mut pool = request.pool();
            let po = pool.allocate(analysis);
            if let Some(analysis) = unsafe { po.as_ref() } {
                ngx_log_debug_http!(request, "strict_sni pool alloc succ");
                request.set_ctx::<StrictSniHttpModule>(analysis);
                if let Some(config) = request.main_conf::<StrictSniHttpModule>() {
                    let val: Validator = config.into();
                    ngx_log_debug_http!(request, "strict_sni why? val: {:?}", config);
                    return match val.validate(request, analysis) {
                        Ok(()) => core::Status::NGX_DECLINED,
                        Err(err_status) => err_status.into(),
                    };
                } else {
                    ngx_log_debug_http!(request, "strict_sni direct filter config nullptr ERR");
                }
            } else {
                ngx_log_debug_http!(request, "strict_sni pool alloc nullptr ERR");
            }
        } else {
            ngx_log_debug_http!(request, "strict_sni analyze fail");
        }

        core::Status::NGX_ERROR

        //let module = module_ref();

        // if let Some(co) =
        //     request.get_module_loc_conf::<ModuleConfig>(unsafe { &*addr_of!(strict_sni_module) })
        // {
        //     ngx_log_debug_http!(request, "strict_sni module status: {:?}", co);
        //     if let Ok(v) = TryInto::<Validator>::try_into((co, MODULE_DATA.get().unwrap())) {
        //         ngx_log_debug_http!(request, "strict_sni module activated: {:?}", v);
        //         match v.validate_request(request) {
        //             Ok(()) => core::Status::NGX_DECLINED,
        //             Err(err_status) => err_status.into(),
        //         }
        //     } else {
        //         ngx_log_debug_http!(request, "strict_sni module off DECL");
        //         core::Status::NGX_DECLINED
        //     }
        // } else {
        //     ngx_log_debug_http!(request, "strict_sni config nullptr ERR");
        //     core::Status::NGX_ERROR
        // }
    }
}

struct PreaccessHandler;
impl HTTPHandler for PreaccessHandler {
    type Module = StrictSniHttpModule;

    const PHASE: Phase = Phase::Preaccess;

    fn handle(request: &mut Request) -> Status {
        ngx_log_debug_http!(request, "strict_sni preaccess_handler called");

        if let Some(config) = request.loc_conf::<StrictSniHttpModule>() {
            ngx_log_debug_http!(request, "strict_sni config: {:?}", config);
            if let Some(analysis) = request.get_ctx::<StrictSniHttpModule>() {
                ngx_log_debug_http!(request, "strict_sni analysis: {:?}", analysis);
                //request.set_ctx::<StrictSniHttpModule>(an);

                let val: Validator = config.into();
                return match val.validate(request, analysis) {
                    Ok(()) => core::Status::NGX_DECLINED,
                    Err(err_status) => err_status.into(),
                };
            } else {
                if request.get_inner().internal() != 0 {
                    // internal redirect not inherit ctx, but it is ok
                    return core::Status::NGX_DECLINED;
                }
                ngx_log_debug_http!(
                    request,
                    "strict_sni analysis nullptr: internal? {}",
                    request.get_inner().internal()
                );
            }
        } else {
            ngx_log_debug_http!(request, "strict_sni config nullptr ERR");
        }
        core::Status::NGX_ERROR
    }
}

// memo:
// nginx won't confuse listening ip and port
// but nginx won't check request (! not host) (host name / port) vs sni / listening port
// here check request
// logic:
// - port check (EASY):
//   - if conn has port:
//     - if req has port: should =
//     - if req not has port: should = scheme default port
//   - if conn not has port:
//     - if req has port: fail
//     - if req not has port: succ
//   - this apply to both request_line port and host_header port
// - host check (HAAAAAAAARD):
//   - if http: cannot check ? !!!!! request line could contain absolute-form !!!!! -> no problem
//   - if https:
//     # here we need ssl connection host name and host header host name should match
//     # but what means "ssl connection host name" and "match"?
//     # ssl connection host name candidate:
//     # - SNI ... most important, but optional
//     # 1. completely subjectname is equal to
//     # 2.
//
//     - if has sni:
//       - if host = sni: succ
//       - if host != sni:
//          # this means fallback connection
//          - if strict: error
//          - if not strict:
//     - if no has sni:
//       # this means fallback connection
//       - if strict: error
//       - if not strict:

#[derive(Debug)]
struct Analysis {
    port_succ_flag: bool,
    host_succ_flag: bool,
}
// impl Drop for Analysis {
//     fn drop(&mut self) {
//         todo!()
//     }
// }

#[derive(Debug)]
struct Analyzer<'a> {
    host: &'a VariableHook,
    scheme: &'a VariableHook,
    sni: &'a VariableHook,
}

impl<'a> From<&'a StrictSniCommon> for Analyzer<'a> {
    fn from(common: &'a StrictSniCommon) -> Self {
        Analyzer {
            host: &common.host,
            scheme: &common.scheme,
            sni: &common.sni,
        }
    }
}
impl<'a> Analyzer<'a> {
    fn get_var_host_str(&self, request: &'a Request) -> Option<&'a str> {
        if let Some(host_slice) = self.host.get(request) {
            return from_utf8(host_slice).ok();
        }
        None
    }
    fn get_var_scheme_str(&self, request: &'a Request) -> Option<&'a str> {
        if let Some(scheme_slice) = self.scheme.get(request) {
            return from_utf8(scheme_slice).ok();
        }
        None
    }
    fn get_var_sni_str(&self, request: &'a Request) -> Option<&'a str> {
        if let Some(sni_slice) = self.sni.get(request) {
            return from_utf8(sni_slice).ok();
        }
        None
    }
    fn analyze(&self, request: &Request) -> Result<Analysis, ()> {
        //let inn: &ngx_http_request_t = request.get_inner();
        // ngx_log_debug_http!(
        //     request,
        //     "uri {} {} {}",
        //     inn.uri,
        //     inn.unparsed_uri,
        //     inn.request_line.to_str(),
        // );
        // //let a = unsafe { inn.upstream.as_ref() }.unwrap();
        // //ngx_log_debug_http!(request, "uri {}", a.uri.to_str(),a.);

        // if let Some(hc) = unsafe { inn.http_connection.as_ref() } {
        //     ngx_log_debug_http!(request, "http_connection");
        //     if let Some(sn) = unsafe { hc.ssl_servername.as_ref() } {
        //         ngx_log_debug_http!(request, "ssl_servername {}", sn);
        //     }
        //     // if let Some(snr) = unsafe { hc.ssl_servername_regex.as_ref() } {
        //     //     ngx_log_debug_http!(request, "ssl_servername_regex {}", snr);
        //     // }
        // }
        // let a = inn.host_start;

        let header_hp = if let Some(hhs) = request.host_header().and_then(|s| s.to_str().ok()) {
            let hp = extract_header_host_port(hhs);
            ngx_log_debug_http!(
                request,
                "strict_sni header parse succ: \"{}\" -> {:?}",
                hhs,
                hp
            );
            hp
        } else {
            None
        };

        let line_hp = if let Some(rls) = request.request_line().and_then(|s| s.to_str().ok()) {
            let hp = extract_line_host_port(rls);
            ngx_log_debug_http!(
                request,
                "strict_sni request line parse succ: \"{}\" -> {:?}",
                rls,
                hp
            );
            hp
        } else {
            None
        };

        //ngx_log_debug_http!(request, "strict_sni port check activated");
        let conn_port = get_local_port(request);
        let scheme_port: Option<u16> = match self.get_var_scheme_str(request) {
            Some(str) => match str {
                "http" => Some(80),
                "https" => Some(443),
                _ => None,
            },
            None => None,
        };
        ngx_log_debug_http!(
            request,
            "strict_sni port: conn:{:?} scheme:{:?}",
            conn_port,
            scheme_port
        );

        let mut port_succ_flag: bool = true;
        if let Some(hp) = header_hp {
            let header_port = hp.1;
            ngx_log_debug_http!(request, "strict_sni port: header:{:?}", header_port);
            port_succ_flag &= validate_port(conn_port, header_port, scheme_port);
        }

        if let Some(hp) = line_hp {
            let line_port = hp.1;
            ngx_log_debug_http!(request, "strict_sni port: line:{:?}", line_port);
            port_succ_flag &= validate_port(conn_port, line_port, scheme_port);
        }

        let mut host_succ_flag: bool = true;
        if let Some(select_host) = self.get_var_host_str(request) {
            ngx_log_debug_http!(request, "strict_sni select_host: {}", select_host);
            if let Some(sni) = self.get_var_sni_str(request) {
                ngx_log_debug_http!(request, "strict_sni sni: {}", sni);
                if !eq_host_name(sni, select_host) {
                    host_succ_flag = false;
                } else {
                }
                // match rigor {
                //     HostCheckRigor::Strict => {}
                //     HostCheckRigor::Normal => {
                //         //unsafe { ngx_http_get_variable(request, name, key) };
                //         let conn_info = get_scheme_info(conn);
                //         match conn_info {
                //             SchemeInfo::Http => {}
                //             SchemeInfo::Https { sni } => todo!(),
                //         };
                //         //validate_all_host_headers(request, |host, port| {}, |_| a);
                //         Ok(())
                //     }
                //     ValidationMode::HostOnly => {
                //         // let (conn_host, _) = get_host_and_scheme(conn);
                //         // let conn_host = conn_host.map(|s| s.to_str());
                //         Ok(())
                //     }
                // }
            }
        }

        Ok(Analysis {
            port_succ_flag,
            host_succ_flag,
        })
    }
}

#[derive(Debug)]
struct Validator<'a> {
    port_mode: Option<()>,
    host_mode: Option<&'a HostCheckRigor>,
}

impl<'a> From<&'a ValidationConfig> for Validator<'a> {
    fn from(conf: &'a ValidationConfig) -> Self {
        let port_mode = match &conf.port_mode {
            CheckSwitch::On(()) => Some(()),
            _ => None,
        };
        let host_mode = match &conf.host_mode {
            CheckSwitch::On(rigor) => Some(rigor),
            _ => None,
        };
        Validator {
            port_mode,
            host_mode,
        }
    }
}

impl Validator<'_> {
    fn validate(&self, request: &Request, analysis: &Analysis) -> Result<(), HTTPStatus> {
        let mut succ_flag = true;
        match &self.port_mode {
            Some(()) => {
                ngx_log_debug_http!(request, "strict_sni port check activated");
                succ_flag &= analysis.port_succ_flag;
            }
            None => (),
        };

        match &self.host_mode {
            Some(rigor) => {
                ngx_log_debug_http!(
                    request,
                    "strict_sni host check activated: rigor: {:?}",
                    rigor
                );
                succ_flag &= analysis.host_succ_flag;
            }
            None => (),
        }

        if !succ_flag {
            return Err(HTTPStatus::MISDIRECTED_REQUEST);
        }

        // for (k, v) in request.headers_in_iterator() {
        //     if k.eq_ignore_ascii_case("host") {
        //         match vaildate_host_header(&v, con_host, con_port, scheme_default_port) {
        //             Ok(()) => (),
        //             Err(status) => return status.into(),
        //         };
        //     }
        // }

        // let conn_port = get_local_port(conn);
        // request.get_inner()
        // core::Status::NGX_DECLINED

        Ok(())
    }
}

fn extract_header_host_port(hhs: &str) -> Option<(&str, Option<u16>)> {
    parse_host_header(hhs).ok()
}

fn extract_line_host_port(rls: &str) -> Option<(&str, Option<u16>)> {
    if let Ok((method, uri, _signature)) = parse_request_line(rls) {
        // if method == connect, then it is new style hop-by-hop request, and then uri not mean the server's host name.
        // if != , then it is old style absolute form request, and then uri mean proxy, and nginx behave as proxy only for internal virtual server.
        // here we don't check http version, since there would be a lot of undocumented extension implementation.
        if !method.eq_ignore_ascii_case("CONNECT") {
            if let Ok(uri) = UriRef::parse(uri) {
                if let Some(auth) = uri.authority() {
                    let host = auth.host();
                    let port = auth.port_to_u16().unwrap_or(None);
                    return Some((host, port));
                }
            }
        }
    }
    None
}

fn get_local_port(request: &Request) -> Option<u16> {
    if let Some(conn) = unsafe { request.connection().as_ref() } {
        if let Some(addr) = unsafe { conn.local_sockaddr.as_mut() } {
            // ngx_inet_get_port is implemented without the use of mutability, so no problem
            let p = unsafe { ngx_inet_get_port(addr) };
            if p != 0 {
                return Some(p);
            }
        }
    }
    None
}

fn validate_port(conn_port: Option<u16>, req_port: Option<u16>, scheme_port: Option<u16>) -> bool {
    if let Some(conn_port) = conn_port {
        if let Some(req_port) = req_port.or(scheme_port) {
            conn_port == req_port
        } else {
            false
        }
    } else {
        if let Some(_) = req_port {
            false
        } else {
            true
        }
    }
}

// #[derive(PartialEq, Eq)]
// struct CertificateFingerprint {
//     raw: [raw::c_uchar; EVP_MAX_MD_SIZE as usize],
// }

// fn get_cert_fingerprint(cert: &X509) -> Option<CertificateFingerprint> {
//     let mut buf: [raw::c_uchar; EVP_MAX_MD_SIZE as usize] = [0; EVP_MAX_MD_SIZE as usize];
//     if unsafe {
//         X509_digest(
//             cert,
//             EVP_sha1(),
//             &mut buf as *mut raw::c_uchar,
//             std::ptr::null_mut(),
//         )
//     } == 1
//     {
//         Some(CertificateFingerprint { raw: buf })
//     } else {
//         None
//     }
// }

// struct SslInfo<'a> {
//     sni: Option<&'a str>,
//     //cert: &'a mut X509,
// }
// impl<'a> SslInfo<'a> {
//     fn check_host(&'a self, name: &str) -> bool {
//         let a: &[raw::c_char] = name;
//         X509_check_host(*cert, a.as_ptr(), a.len(), 0, std::ptr::null_mut());
//         true
//     }
// }

// fn get_ssl_info<'a>(request: &Request, conn: &'a ngx_connection_t) -> Option<SslInfo<'a>> {
//     ngx_log_debug_http!(request, "load conn");
//     if let Some(ssl) = unsafe { conn.ssl.as_ref() } {
//         // if let Some(ssl_conn) = unsafe { ssl.connection.as_ref() } {
//         //     if let Some(cert) = unsafe { SSL_get_certificate(ssl_conn).as_ref() } {
//         //         get_cert_fingerprint(cert);
//         //     }
//         // }
//         // let a: ngx::ffi::ngx_http_connection_t;

//         let mut name = ngx_null_string!();
//         let sni = if unsafe {
//             // here we get str alloc in conn pool: this means &str lifetime is same with conn.
//             ngx_ssl_get_server_name(conn, conn.pool, &mut name)
//         } == Status::NGX_OK.into()
//         {
//             // ngx_log_debug_http!(request, "strict_sni ssl_servername \"{}\"", sni);
//             // here unsafely expand the lifetime, but it is actually vaild.
//             let sni: &'a str = unsafe { (name.to_str() as *const str).as_ref().unwrap() };
//             Some(sni)
//         } else {
//             // ngx_log_debug_http!(request, "strict_sni ssl_servername nullptr");
//             None
//         };
//         Some(SslInfo { sni })
//     } else {
//         None
//     }
// }

// fn get_scheme_str<'a>(request: &'a Request) -> Option<&'a str> {
//     let inner = request.get_inner();
//     if !(inner.schema_start.is_null() || inner.schema_end.is_null()) {
//         let sig_size = unsafe { inner.schema_end.offset_from(inner.schema_start) };
//         ngx_log_debug_http!(request, "scheme sig_size: {}", sig_size);
//         if sig_size >= 0 {
//             let size = sig_size as usize;
//             let ptr: &[u8] = unsafe { std::slice::from_raw_parts(inner.schema_start, size) };
//             let schema_str: &str = unsafe { from_utf8_unchecked(ptr) };
//             return Some(schema_str);
//         }
//     }
//     None
// }

// fn get_port_str<'a>(request: &'a Request) -> Option<&'a str> {
//     let inner = request.get_inner();
//     ngx_log_debug_http!(request, "port: {:?}={:?}", inner.port_start, inner.port_end);
//     if !(inner.port_start.is_null() || inner.port_end.is_null()) {
//         let sig_size = unsafe { inner.port_end.offset_from(inner.port_start) };
//         ngx_log_debug_http!(request, "port sig_size: {}", sig_size);
//         if sig_size >= 0 {
//             let size = sig_size as usize;
//             ngx_log_debug_http!(request, "size: {}", size);
//             let ptr: &[u8] = unsafe { std::slice::from_raw_parts(inner.port_start, size) };
//             let port_str: &str = unsafe { from_utf8_unchecked(ptr) };
//             return Some(port_str);
//         }
//     }
//     None
// }

fn eq_host_name(host1: &str, host2: &str) -> bool {
    host1.eq_ignore_ascii_case(host2)
}
