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

# # install
# echo install
# if [ "$(uname -s)" = 'Darwin' ];then
#     brew update
#     # we already have them
# else
#     if which apt;then
#         apt-get -y update
#         apt-get -y install curl build-essential libclang-dev
#     elif which yum;then
#         yum -y update
#         yum -y install curl build-essential libclang-dev
#     fi
# fi
# if [ $? -ne 0 ];then
#     exit 3
# fi

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

echo % case 1: host valid url valid
s1=$(curl -H "Host: localhost" -k https://localhost -o /dev/null -w '%{http_code}\n' -s)
echo % case 2: host valid url invalid
s2=$(curl -H "Host: localhost" -k https://localhost/xxx -o /dev/null -w '%{http_code}\n' -s)
echo % case 3: host invalid url valid
s3=$(curl -H "Host: localguest" -k https://localhost -o /dev/null -w '%{http_code}\n' -s)
echo % case 4: host invalid url invalid
s4=$(curl -H "Host: localguest" -k https://localhost/xxx -o /dev/null -w '%{http_code}\n' -s)
echo % case 5: host null url valid
s5=$(curl -k https://localhost -o /dev/null -w '%{http_code}\n' -s)
echo % case 6: host null url invalid
s6=$(curl -k https://localhost/xxx -o /dev/null -w '%{http_code}\n' -s)

# test finale
".nginx/$ngxver/$target/sbin/nginx" -s stop
if [ $s1 -ne 200 ];then
    exit 61
fi
if [ $s2 -ne 404 ];then
    exit 62
fi
if [ $s3 -ne 421 ];then
    exit 63
fi
if [ $s4 -ne 421 ];then
    exit 64
fi
if [ $s5 -ne 200 ];then
    exit 65
fi
if [ $s6 -ne 404 ];then
    exit 66
fi
