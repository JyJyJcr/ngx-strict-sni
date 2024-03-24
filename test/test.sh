#!/bin/sh

# validate
echo vaildate
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

# install
echo install
if [ "$(uname -s)" = 'Darwin' ];then
    brew update
    # we already have them
else
    if which apt;then
        sudo apt-get -y update
        sudo apt-get -y install curl build-essential libclang-dev
    elif which yum;then
        sudo yum -y update
        sudo yum -y install curl build-essential libclang-dev
    fi
fi
if [ $? -ne 0 ];then
    exit 3
fi

echo install rust

# install rust
if ! which cargo; then
    export PATH="${HOME}/.cargo/bin:$PATH"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi

if [ $? -ne 0 ];then
    exit 4
fi

echo build

# build
export NGX_VERSION="$ngxver"
cargo update
cargo build --target "$target"

if [ $? -ne 0 ];then
    exit 6
fi

echo test

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
