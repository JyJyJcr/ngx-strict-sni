# ngx-strict-sni

Strict SNI validator for Nginx

## Abstruct

The ngx_http_ssl_strict_sni module is a validator of the integrity of SNI and the Host header. This blocks "SNI spoofing" to virtual hosts. Without ssl, this module has no effects.

## Description

Nginx doesn't and won't check the integrity of SNI and Host header, resulting to allowing "SNI spoofing" between virtual hosts.

Example: let two virtual host `aaa.com` and `bbb.com` listen on 443 as https servers with their names.

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

`/srv/www/com.aaa/foo.txt`

```html
Here is aaa.com.
```

`/srv/www/com.bbb/foo.txt`

```html
Here is bbb.com.
```

By normal requests, we get `/foo.txt` of the server specified in domain:

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

This module adds the validatior of SNI and Host header, and when the request violate the rule, return Status 421 Misdirected Request:

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

Currently, only the apt repository for debian bookworm is available. The package name is changed to `libnginx-mod-http-ssl-strict-sni` following the standard name style of Nginx modules.

```bash
sudo echo "deb [trusted=yes] https://jyjyjcr.github.io/ngx-strict-sni/publish/apt bookworm main" > "/etc/apt/sources.list.d/ngx-strict-sni.list"
apt update
apt install libnginx-mod-http-ssl-strict-sni
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
