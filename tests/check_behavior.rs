#[cfg(test)]
mod tests {
    use ngx::test_util::{target_path, Nginx};
    use std::env::{consts::*, current_dir};

    const TEST_NGINX_CONF: &str = "tests/nginx.conf";
    const TEST_NGINX_PEM: &str = "tests/nginx.pem";
    const TEST_NGINX_KEY: &str = "tests/nginx.key";

    const TEST_CURL_TUPLE: [(&str, Option<&str>, u32); 32] = [
        // ssl root
        ("https://localhost:4433", None, 200),
        ("https://localhost:4433", Some("localhost:4433"), 200),
        ("https://localhost:4433", Some("localguest:4433"), 421),
        ("https://localhost:4433", Some("localhost:4422"), 421),
        // ssl unexist sub
        ("https://localhost:4433/xxx", None, 404),
        ("https://localhost:4433/xxx", Some("localhost:4433"), 404),
        ("https://localhost:4433/xxx", Some("localguest:4433"), 421),
        ("https://localhost:4433/xxx", Some("localhost:4422"), 421),
        // ssl strict sub
        ("https://localhost:4433/sub", None, 301),
        ("https://localhost:4433/sub", Some("localhost:4433"), 301),
        ("https://localhost:4433/sub", Some("localguest:4433"), 421),
        ("https://localhost:4433/sub", Some("localhost:4422"), 421),
        // ssl dull sub
        ("https://localhost:4433/dull", None, 301),
        ("https://localhost:4433/dull", Some("localhost:4433"), 301),
        ("https://localhost:4433/dull", Some("localguest:4433"), 301),
        ("https://localhost:4433/dull", Some("localhost:4422"), 301),
        // bare root
        ("http://localhost:8080", None, 200),
        ("http://localhost:8080", Some("localhost:8080"), 200),
        ("http://localhost:8080", Some("localguest:8080"), 200),
        ("http://localhost:8080", Some("localhost:8888"), 200),
        // bare unexist sub
        ("http://localhost:8080/xxx", None, 404),
        ("http://localhost:8080/xxx", Some("localhost:8080"), 404),
        ("http://localhost:8080/xxx", Some("localguest:8080"), 404),
        ("http://localhost:8080/xxx", Some("localhost:8888"), 404),
        // bare strict sub
        ("http://localhost:8080/sub", None, 301),
        ("http://localhost:8080/sub", Some("localhost:8080"), 301),
        ("http://localhost:8080/sub", Some("localguest:8080"), 301),
        ("http://localhost:8080/sub", Some("localhost:8888"), 301),
        // bare dull sub
        ("http://localhost:8080/dull", None, 301),
        ("http://localhost:8080/dull", Some("localhost:8080"), 301),
        ("http://localhost:8080/dull", Some("localguest:8080"), 301),
        ("http://localhost:8080/dull", Some("localhost:8888"), 301),
    ];

    #[test]
    fn test() {
        // get nginx controller
        let mut nginx = Nginx::default();

        // search conf and module
        let current_dir = current_dir().expect("Unable to get current directory");
        let test_config_path = current_dir.join(TEST_NGINX_CONF);
        let test_pem_path = current_dir.join(TEST_NGINX_PEM);
        let test_key_path = current_dir.join(TEST_NGINX_KEY);
        let module_basename = format!(
            "{}{}{}",
            DLL_PREFIX,
            env!("CARGO_PKG_NAME").replace('-', "_"),
            DLL_SUFFIX
        );
        let module_path = target_path(&module_basename).expect("target dir not found");

        assert!(
            test_config_path.is_file(),
            "Config file not found: {}\nCurrent directory: {}",
            test_config_path.to_string_lossy(),
            current_dir.to_string_lossy()
        );
        assert!(
            module_path.is_file(),
            "Module not found: {}\nCurrent directory: {}",
            module_path.to_string_lossy(),
            current_dir.to_string_lossy()
        );
        assert!(
            test_pem_path.is_file(),
            "PEM file not found: {}\nCurrent directory: {}",
            test_pem_path.to_string_lossy(),
            current_dir.to_string_lossy()
        );
        assert!(
            test_key_path.is_file(),
            "KEY file not found: {}\nCurrent directory: {}",
            test_key_path.to_string_lossy(),
            current_dir.to_string_lossy()
        );

        // put them into the nginx dir
        nginx
            .copy_config(&test_config_path)
            .expect(format!("Unable to load config file: {}", test_config_path.display()).as_str());
        nginx
            .copy_config(&test_pem_path)
            .expect(format!("Unable to load PEM file: {}", test_pem_path.display()).as_str());
        nginx
            .copy_config(&test_key_path)
            .expect(format!("Unable to load KEY file: {}", test_key_path.display()).as_str());
        nginx
            .copy_module(&module_path)
            .expect(format!("Unable to load module dylib: {}", module_path.display()).as_str());
        nginx
            .create_config_from_str(
                "load_module.conf",
                "load_module modules/libngx_strict_sni.dylib;",
            )
            .expect(format!("Unable to create config file").as_str());

        // start nginx
        let output = nginx.restart().expect("Unable to restart NGINX");
        assert!(output.status.success());

        // test core
        let test_result = TEST_CURL_TUPLE
            .map(|(url, header_host, code)| (url, header_host, code, curl_test(url, header_host)));

        // stop nginx
        let output = nginx.stop().expect("Unable to stop NGINX");
        assert!(output.status.success());

        // test valid
        for (url, header_host, code, res) in test_result {
            let res = res.unwrap();
            if res != code {
                panic!(
                    "url: {}, header: {:?}, expected:{} != ans:{}",
                    url, header_host, code, res
                )
            }
        }
    }

    use curl::{
        easy::{Easy, List},
        Error,
    };
    fn curl_test(url: &str, header_host: Option<&str>) -> Result<u32, Error> {
        let mut list = List::new();
        if let Some(hh) = header_host {
            list.append(format!("Host: {}", hh).as_str())?;
        }
        let mut handle = Easy::new();
        handle.ssl_verify_peer(false)?;
        handle.ssl_verify_host(false)?;
        handle.url(url)?;
        handle.http_headers(list)?;
        handle.perform()?;
        return Ok(handle.response_code()?);
    }
}
