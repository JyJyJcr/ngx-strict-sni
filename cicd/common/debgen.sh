#!/bin/sh
if [ -z "$1" ];then
    exit 70
fi
cargo deb --variant $1
for deb in $(ls target/debian|grep -E '\.deb$') ;do
    cat /etc/os-release|grep VERSION_CODENAME|sed -e "s/^.*=//g" > target/debian/$deb.codename
done
