#[cfg(test)]
mod tests {
    use ngx::test_util::{target_path, Nginx};
    use std::env::{consts::*, current_dir};

    const TEST_NGINX_CONFIG: &str = "tests/nginx.conf";

    #[test]
    fn test() {
        let mut nginx = Nginx::default();

        let current_dir = current_dir().expect("Unable to get current directory");
        let test_config_path = current_dir.join(TEST_NGINX_CONFIG);
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
            test_config_path.is_file(),
            "Module not found: {}\nCurrent directory: {}",
            test_config_path.to_string_lossy(),
            current_dir.to_string_lossy()
        );
        assert!(
            test_config_path.is_file(),
            "Config file not found: {}\nCurrent directory: {}",
            test_config_path.to_string_lossy(),
            current_dir.to_string_lossy()
        );

        nginx
            .replace_config(&test_config_path)
            .expect(format!("Unable to load config file: {}", test_config_path.display()).as_str());
        nginx
            .copy_module(&module_path)
            .expect(format!("Unable to load module dylib: {}", module_path.display()).as_str());
        nginx
            .create_config_from_str(
                "load_module.conf",
                "load_module modules/libngx_strict_sni.dylib;",
            )
            .expect(format!("Unable to create config file").as_str());
        let output = nginx.restart().expect("Unable to restart NGINX");
        assert!(output.status.success());

        let output = nginx.stop().expect("Unable to stop NGINX");
        assert!(output.status.success());
    }
}
