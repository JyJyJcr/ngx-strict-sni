#!/bin/sh

# valid
if [ -z "$1" ];then
    exit 1
else
    ngxver=$1
fi
if [ -z "$2" ];then
    exit 2
else
    target=$2
fi

# install curl
if ! which curl; then
    if [ "$(uname -s)" == 'Darwin' ];then
        brew install curl
    else
        if which apt;then
            apt install curl
        elif which yum;then
            yum install curl
        fi
    fi
fi

# install rust
if ! which cargo; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    export PATH="${HOME}/.cargo/bin:$PATH"
fi

# build
export NGX_VERSION="$ngxver"
cargo update
cargo build --target "$target"

# test preparation
cp test/test.conf ".nginx/$ngxver/$target/conf/nginx.conf"
cd "target/$target/debug"
for lib in $(ls|grep -E "ngx_strict_sni\.(dylib|so)");do
    cp "$lib" "../../../.nginx/$ngxver/$target/"
    echo "load_module $lib;" > "../../../.nginx/$ngxver/$target/conf/load_module.conf"
done
cd -
".nginx/$ngxver/$target/sbin/nginx"

# test
echo
echo % case 1: host valid url valid
echo
curl -H "Host: localhost" -k https://localhost
echo
echo % case 2: host valid url invalid
echo
curl -H "Host: localhost" -k https://localhost/xxx
echo
echo % case 3: host invalid url valid
echo
curl -H "Host: localguest" -k https://localhost
echo
echo % case 4: host invalid url invalid
echo
curl -H "Host: localguest" -k https://localhost/xxx
echo
echo % case 5: host null url valid
echo
curl -k https://localhost
echo
echo % case 6: host null url invalid
echo
curl -k https://localhost/xxx

# test finale
".nginx/$ngxver/$target/sbin/nginx" -s stop
