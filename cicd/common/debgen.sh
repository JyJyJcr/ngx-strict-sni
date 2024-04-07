#!/bin/sh
if [ -z "$1" ];then
    exit 70
fi
if [ -z "$2" ];then
    exit 71
fi
cargo deb --target $2 --variant $1
for deb in $(ls target/debian|grep -E '\.deb$') ;do
    cat /etc/os-release|grep VERSION_CODENAME|sed -e "s/^.*=//g" > target/debian/$deb.codename
done
