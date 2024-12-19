mod util;

use ::core::str;
use fluent_uri::{Uri, UriRef};
use ngx::ffi::{
    nginx_version, ngx_array_push, ngx_command_t, ngx_conf_s, ngx_conf_t, ngx_connection_t,
    ngx_http_core_module, ngx_http_get_flushed_variable, ngx_http_get_variable,
    ngx_http_get_variable_index, ngx_http_get_variable_pt, ngx_http_handler_pt, ngx_http_module_t,
    ngx_http_phases_NGX_HTTP_ACCESS_PHASE, ngx_http_phases_NGX_HTTP_POST_READ_PHASE,
    ngx_http_phases_NGX_HTTP_PREACCESS_PHASE, ngx_http_request_t, ngx_http_ssl_certificate,
    ngx_inet_get_port, ngx_int_t, ngx_module_t, ngx_ssl_certificate_index, ngx_ssl_get_server_name,
    ngx_ssl_get_subject_dn, ngx_str_t, ngx_uint_t, EVP_sha1, SSL_get0_peer_certificate,
    SSL_get_certificate, X509_LOOKUP_by_fingerprint, X509_check_host, X509_digest, X509_free,
    X509_get_serialNumber, EVP_MAX_MD_SIZE, NGX_CONF_TAKE1, NGX_ERROR, NGX_HTTP_LOC_CONF,
    NGX_HTTP_MAIN_CONF, NGX_HTTP_MODULE, NGX_HTTP_SRV_CONF, NGX_HTTP_VERSION_20,
    NGX_RS_HTTP_LOC_CONF_OFFSET, NGX_RS_MODULE_SIGNATURE, X509,
};
use ngx::http::{HTTPStatus, MergeConfigError, Request};
use ngx::{core, core::Status, http, http::HTTPModule};
use ngx::{
    http_request_handler, ngx_log_debug_http, ngx_modules, ngx_null_command, ngx_null_string,
    ngx_string,
};
use std::cell::OnceCell;
use std::num::ParseIntError;
use std::os::raw::{self, c_char, c_void};
use std::ptr::{addr_of, slice_from_raw_parts, slice_from_raw_parts_mut};
use std::slice::from_raw_parts;
use std::str::{from_utf8, from_utf8_unchecked};
use std::sync::OnceLock;
use std::{default, slice};
use util::{get_host_header_str, get_request_line_str, parse_host_header, parse_request_line};

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

static MODULE_DATA: OnceLock<ModuleCommon> = OnceLock::new();

#[derive(Debug)]
struct ModuleCommon {
    host: VariableRef,
    scheme: VariableRef,
    sni: VariableRef,
}

#[derive(Debug)]
struct VariableRef(ngx_uint_t);
fn get_variable_ref(cf: &mut ngx_conf_t, name: &mut ngx_str_t) -> Option<VariableRef> {
    let r = unsafe { ngx_http_get_variable_index(cf, name) };
    if r == NGX_ERROR as ngx_int_t {
        None
    } else {
        Some(VariableRef(r as ngx_uint_t))
    }
}

fn solve_variable_ref<'a>(r: &VariableRef, req: &'a Request) -> Option<&'a [u8]> {
    let r = unsafe { ngx_http_get_flushed_variable(req.get_inner() as *const _ as *mut _, r.0) };
    if let Some(v) = unsafe { r.as_ref() } {
        if v.not_found() == 0 {
            let ptr = slice_from_raw_parts(v.data, v.len() as usize);
            if let Some(slice) = unsafe { ptr.as_ref() } {
                return Some(slice);
            }
        }
    }
    None
}
// fn solve_variable_ref_mut<'a>(r: &VariableRef,req:&'a mut Request)->Option<&'a mut [u8]>{
//     let r = unsafe { ngx_http_get_flushed_variable( req.get_inner() as *const _ as *mut _, r.0) };
//     if let Some(v) =unsafe{r.as_ref()} {
//         if v.not_found() == 0 {
//             let ptr=slice_from_raw_parts(v.data, v.len() as usize);
//             if let Some(slice)=unsafe{ptr.as_ref()}{
//                 return Some(slice)
//             }
//         }
//     }
//     None
// }

impl http::HTTPModule for Module {
    type MainConf = ();
    type SrvConf = ();
    type LocConf = ModuleConfig;
    unsafe extern "C" fn postconfiguration(cf: *mut ngx_conf_t) -> ngx_int_t {
        if let Some(cf) = unsafe { cf.as_mut() } {
            if let Some(conf) = unsafe {
                http::ngx_http_conf_get_module_main_conf(cf, &*addr_of!(ngx_http_core_module))
                    .as_mut()
            } {
                if let Some(pointer) = unsafe {
                    (ngx_array_push(
                        &mut conf.phases[ngx_http_phases_NGX_HTTP_PREACCESS_PHASE as usize]
                            .handlers,
                    ) as *mut ngx_http_handler_pt)
                        .as_mut()
                } {
                    *pointer = Some(strict_sni_access_handler);
                    if let Some(vr_host) = get_variable_ref(cf, &mut ngx_string!("host")) {
                        if let Some(vr_scheme) = get_variable_ref(cf, &mut ngx_string!("scheme")) {
                            if let Some(vr_sni) =
                                get_variable_ref(cf, &mut ngx_string!("ssl_server_name"))
                            {
                                let _ = MODULE_DATA.set(ModuleCommon {
                                    host: vr_host,
                                    scheme: vr_scheme,
                                    sni: vr_sni,
                                });
                                return Status::NGX_OK.into();
                            }
                        }
                    };
                }
            }
        }
        Status::NGX_ERROR.into()
    }
}

#[derive(Debug, Default)]
struct ModuleConfig {
    //rfc_mode: RfcCheckMode,
    port_mode: PortCheckMode,
    host_mode: HostCheckMode,
}

#[derive(Debug)]
struct Validator {
    port_mode: Option<()>,
    host_mode: Option<HostCheckRigor>,
    common: &'static ModuleCommon,
}

impl ModuleConfig {
    fn get_validator(&self, common_cell: &'static OnceLock<ModuleCommon>) -> Option<Validator> {
        let port_mode = match self.port_mode {
            PortCheckMode::On => Some(()),
            _ => None,
        };
        let host_mode = match &self.host_mode {
            HostCheckMode::On(rigor) => Some(rigor.clone()),
            _ => None,
        };
        if port_mode.is_some() || host_mode.is_some() {
            if let Some(common) = common_cell.get() {
                return Some(Validator {
                    port_mode,
                    host_mode,
                    common,
                });
            }
        }
        None
    }
}

#[derive(Debug, Default, Clone)]
enum PortCheckMode {
    #[default]
    Unset,
    Off,
    On,
}
impl PortCheckMode {
    fn is_active(&self) -> bool {
        match self {
            PortCheckMode::Unset | PortCheckMode::Off => false,
            _ => true,
        }
    }
}

#[derive(Debug, Default, Clone)]
enum HostCheckMode {
    #[default]
    Unset,
    Off,
    On(HostCheckRigor),
}

#[derive(Debug, Clone)]
enum HostCheckRigor {
    Normal,
    Strict,
}

impl HostCheckMode {
    fn is_active(&self) -> bool {
        match self {
            HostCheckMode::Unset | HostCheckMode::Off => false,
            _ => true,
        }
    }
}

impl http::Merge for ModuleConfig {
    fn merge(&mut self, prev: &ModuleConfig) -> Result<(), MergeConfigError> {
        if let PortCheckMode::Unset = self.port_mode {
            self.port_mode = prev.port_mode.clone();
        };
        if let HostCheckMode::Unset = self.host_mode {
            self.host_mode = prev.host_mode.clone();
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
    if let Some(conf) = unsafe { (conf as *mut ModuleConfig).as_mut() } {
        if let Some(cf) = unsafe { cf.as_ref() } {
            if let Some(args) = unsafe { cf.args.as_ref() } {
                if let Some(ngx_arg) = unsafe { (args.elts as *mut ngx_str_t).add(1).as_ref() } {
                    let arg = ngx_arg.to_str();
                    // good old on/off
                    if arg.eq_ignore_ascii_case("on") {
                        conf.port_mode = PortCheckMode::On;
                        conf.host_mode = HostCheckMode::On(HostCheckRigor::Normal);
                    } else if arg.eq_ignore_ascii_case("off") {
                        conf.port_mode = PortCheckMode::Off;
                        conf.host_mode = HostCheckMode::Off;
                    } else if arg.eq_ignore_ascii_case("strict") {
                        conf.port_mode = PortCheckMode::On;
                        conf.host_mode = HostCheckMode::On(HostCheckRigor::Strict);
                    }
                    // port only setting
                    if arg.eq_ignore_ascii_case("port") {
                        conf.port_mode = PortCheckMode::On;
                    } else if arg.eq_ignore_ascii_case("no_port") {
                        conf.port_mode = PortCheckMode::Off;
                    }
                    // host only setting
                    if arg.eq_ignore_ascii_case("host") {
                        conf.host_mode = HostCheckMode::On(HostCheckRigor::Normal);
                    } else if arg.eq_ignore_ascii_case("strict_host") {
                        conf.host_mode = HostCheckMode::On(HostCheckRigor::Strict);
                    } else if arg.eq_ignore_ascii_case("no_host") {
                        conf.host_mode = HostCheckMode::Off;
                    }
                };
            }
        }
    }
    std::ptr::null_mut()
}

http_request_handler!(strict_sni_access_handler, |request: &mut http::Request| {
    ngx_log_debug_http!(request, "strict_sni module called");
    if let Some(co) =
        unsafe { request.get_module_loc_conf::<ModuleConfig>(&*addr_of!(strict_sni_module)) }
    {
        ngx_log_debug_http!(request, "strict_sni module status: {:?}", co);
        if let Some(v) = co.get_validator(&MODULE_DATA) {
            ngx_log_debug_http!(request, "strict_sni module activated: {:?}", v);
            match unsafe { request.connection().as_mut() } {
                Some(conn) => {
                    ngx_log_debug_http!(request, "strict_sni connection() succeed");
                    match v.validate_request(conn, request) {
                        Ok(()) => core::Status::NGX_OK,
                        Err(err_status) => err_status.into(),
                    }
                }
                None => {
                    ngx_log_debug_http!(request, "strict_sni connection() nullptr");
                    core::Status::NGX_ERROR
                }
            }
        } else {
            // ngx_log_debug_http!(request, "strict_sni module off");
            core::Status::NGX_DECLINED
        }
    } else {
        ngx_log_debug_http!(request, "strict_sni config nullptr");
        core::Status::NGX_ERROR
    }
});

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

impl Validator {
    fn get_var_host_str<'a>(&self, request: &'a Request) -> Option<&'a str> {
        if let Some(host_slice) = solve_variable_ref(&self.common.host, request) {
            return from_utf8(host_slice).ok();
        }
        None
    }
    fn get_var_scheme_str<'a>(&self, request: &'a Request) -> Option<&'a str> {
        if let Some(scheme_slice) = solve_variable_ref(&self.common.scheme, request) {
            return from_utf8(scheme_slice).ok();
        }
        None
    }
    fn get_var_sni_str<'a>(&self, request: &'a Request) -> Option<&'a str> {
        if let Some(sni_slice) = solve_variable_ref(&self.common.sni, request) {
            return from_utf8(sni_slice).ok();
        }
        None
    }
    #[inline(always)]
    fn validate_request(
        &self,
        conn: &ngx_connection_t,
        request: &Request,
    ) -> Result<(), HTTPStatus> {
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

        let header_hp = if let Some(hhs) = get_host_header_str(request) {
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

        let line_hp = if let Some(rls) = get_request_line_str(request) {
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

        match &self.port_mode {
            Some(()) => {
                ngx_log_debug_http!(request, "strict_sni port check activated");
                let conn_port = get_local_port(conn);
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

                let mut succ_flag: bool = true;
                if let Some(hp) = header_hp {
                    let header_port = hp.1;
                    ngx_log_debug_http!(request, "strict_sni port: header:{:?}", header_port);
                    succ_flag &= validate_port(conn_port, header_port, scheme_port);
                }

                if let Some(hp) = line_hp {
                    let line_port = hp.1;
                    ngx_log_debug_http!(request, "strict_sni port: line:{:?}", line_port);
                    succ_flag &= validate_port(conn_port, line_port, scheme_port);
                }
                if !succ_flag {
                    return Err(HTTPStatus::MISDIRECTED_REQUEST);
                }
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
                if let Some(select_host) = self.get_var_host_str(request) {
                    ngx_log_debug_http!(request, "strict_sni select_host: {}", select_host);
                    if let Some(sni) = self.get_var_sni_str(request) {
                        ngx_log_debug_http!(request, "strict_sni sni: {}", sni);
                        if !eq_host_name(sni, select_host) {
                            return Err(HTTPStatus::MISDIRECTED_REQUEST);
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
            }
            None => (),
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

fn get_local_port(conn: &ngx_connection_t) -> Option<u16> {
    if let Some(addr) = unsafe { conn.local_sockaddr.as_mut() } {
        // ngx_inet_get_port is implemented without the use of mutability, so no problem
        let p = unsafe { ngx_inet_get_port(addr) };
        if p != 0 {
            return Some(p);
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
