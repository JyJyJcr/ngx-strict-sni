# ngx-strict-sni

Strict SNI validator for Nginx

## Abstruct

The ngx_http_ssl_strict_sni module is a validator of the integrity of SNI and the Host header. This blocks "SNI spoofing" to virtual hosts. Without ssl, this module has no effects.

## Description

Nginx doesn't and won't check the integrity of SNI and Host header, resulting to allowing "SNI spoofing" between virtual hosts.

Reproduction: let two virtual host `aaa.com` and `bbb.com` listen on 443 as https servers with their names.

`nginx.conf`:

```nginx
server{
    server_name aaa.com;
    ...
    root /srv/www/com.aaa;
}
server{
    server_name bbb.com;
    ...
    root /srv/www/com.bbb;
    strict_sni: on; # here we enable this module
}
```

`/srv/www/com.aaa/foo.txt`:

```html
Here is aaa.com.
```

`/srv/www/com.bbb/foo.txt`:

```html
Here is bbb.com.
```

With normal requests, we get `/foo.txt` in the server specified in the domain:

```bash
$ curl https://aaa.com/foo.txt
Here is aaa.com.
$ curl https://bbb.com/foo.txt
Here is bbb.com.
```

However, we cat get `aaa.com/foo.txt` with URL `bbb.com/foo.txt`, by adding false `Host` header:

```bash
$ curl -H "Host: aaa.com" https://bbb.com/foo.txt
Here is aaa.com. # <-- intruded!
```

This module adds the validation step of SNI and Host header, and when the request violate the rule, it immediately return Status 421 Misdirected Request:

```bash
$ curl -H "Host: bbb.com" https://aaa.com/foo.txt
<html>
<head><title>421 Misdirected Request</title></head>
<body>
<center><h1>421 Misdirected Request</h1></center>
<hr><center>nginx</center>
</body>
</html>
```

This explanation is written based on the article below:

[NGINXリバースプロキシでTLS Server Name Indication (SNI)と異なるドメイン名のバックエンドホストへルーティングできちゃう件について](https://qiita.com/jqtype/items/bd6f0d819944ef954d88)

## Installation

Currently, only the Debian repository for Debian is available. The package name is changed to `libnginx-mod-http-ssl-strict-sni` following the standard name style of Nginx modules.

### Debian

Supported Codename: `bullseye`, `bookworm`

Supported Architecture: `arm64`, `amd64`

> [!IMPORTANT]
> 2024/03/28: PGP repository signature added! Existing user should update their apt configuration.

> [!IMPORTANT]
> 2024/04/13: Repository URL changed! Existing user should update the URL written in `ngx-strict-sni.list`.

```bash
curl -fsSL https://jyjyjcr.github.io/ngx-strict-sni/publish/gpg.key.asc | sudo gpg --dearmor -o /etc/apt/keyrings/ngx-strict-sni.gpg
sudo echo "deb [signed-by=/etc/apt/keyrings/ngx-strict-sni.gpg] https://jyjyjcr.github.io/ngx-strict-sni/publish/deb $(cat /etc/os-release|grep VERSION_CODENAME|sed -e 's/^.*=//g') main" > "/etc/apt/sources.list.d/ngx-strict-sni.list"
sudo apt update
sudo apt install libnginx-mod-http-ssl-strict-sni
```

## Directives

### `strict_sni`

Syntax: `strict_sni on | off;`

Default: `strict_sni off;`

Context: `http`, `server`, `location`

## Use Case

```nginx
http {
    ...
    strict_sni on; # enable the validator for any https request
    ...
    server{
        server_name strict.com;
        ...
    }
    server{
        server_name loose.com;
        ...
        strict_sni off; # but disable for any https request aimed (not by SNI but Host header) to loose.com
        ...
        location /strict/ {
            strict_sni on; # and re-enable for any to loose.com/strict/
            ...
        }
    }
}
```

## Technology

This module is written in Rust using [ngx](https://crates.io/crates/ngx/0.4.1) crate. The original repository is [here](https://github.com/nginxinc/ngx-rust), and the modified one is [here](https://github.com/JyJyJcr/ngx-rust/tree/integ_test_inuse).
