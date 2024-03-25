# ngx-strict-sni

An Nginx module strictly validating SNI == Host.

## abstruct

Nginx doesn't and won't check the integrity of SNI and Host header.
This module adds the validation stage of them, and when the request violate the rule, return Status 421 Misdirected Request.

## More information

You can easily try it on your own Nginx.

## usage

```nginx
strict_sni on;
strict_sni off;
```
