#!/bin/sh

# validate
echo vaildate
if [ -z "$1" ];then
    exit 1
else
    target=$1
fi
if [ -z "$2" ];then
    exit 2
else
    ngxver=$2
fi
if [ -z "$3" ];then
    exit 3
else
    triple=$3
fi
if [ "$4" = "release" ];then
    is_release=yes
else
    is_release=no
fi

if [ "$5" = "compat" ];then
    is_compat=yes
else
    is_compat=no
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

# # install rust
# echo install rust
# if ! which cargo; then
#     export PATH="${HOME}/.cargo/bin:$PATH"
#     curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
# fi
# if [ $? -ne 0 ];then
#     exit 4
# fi

if rustup default|grep $triple;then
    ign_triple=yes
else
    ign_triple=no
fi

# build
echo build
export NGX_VERSION="$ngxver"
if [ "$is_compat" = "yes" ];then
    export OPENSSL_VERSION="1.1.1w"
    export PCRE2_VERSION=8.45
fi
cargo update
if [ "$ign_triple" = "yes" ];then
    cargo build --release
    date
    ls -l target/release
else
    cargo build --target "$triple" --release
    date
    ls -l target/$triple/release
fi
if [ $? -ne 0 ];then
    exit 6
fi

# test
echo test
# test preparation
cp cicd/test.conf ".nginx/$ngxver/$triple/conf/nginx.conf"
if [ "$ign_triple" = "yes" ];then
    cd "target/release"
    for lib in $(ls|grep -E "ngx_strict_sni\.(dylib|so)");do
        cp "$lib" "../../.nginx/$ngxver/$triple/"
        echo "load_module $lib;" > "../../.nginx/$ngxver/$triple/conf/load_module.conf"
    done
    cd -
else
    cd "target/$triple/release"
    for lib in $(ls|grep -E "ngx_strict_sni\.(dylib|so)");do
        cp "$lib" "../../../.nginx/$ngxver/$triple/"
        echo "load_module $lib;" > "../../../.nginx/$ngxver/$triple/conf/load_module.conf"
    done
    cd -
fi
".nginx/$ngxver/$triple/sbin/nginx"

# test
can=-1
for url in https://localhost:4433/ https://localhost:4433/xxx; do
    for host in localhost localguest; do
        for port in 4433 4422; do
            can=$(expr $can + 1)
            ret=$(curl -H "Host: $host:$port" -k $url -o /dev/null -w '%{http_code}\n' -s)
            echo "case $can: Host=$host:$port url=$url -> $ret"
            eval "ret$can=$ret"
        done
    done
    can=$(expr $can + 1)
    ret=$(curl -k $url -o /dev/null -w '%{http_code}\n' -s)
    echo "case $can: Host..null url=$url -> $ret"
    eval "ret$can=$ret"
done

# test finale
".nginx/$ngxver/$triple/sbin/nginx" -s stop

if [ $ret0 -ne 200 ];then
     exit 60
fi
if [ $ret1 -ne 421 ];then
     exit 61
fi
if [ $ret2 -ne 421 ];then
     exit 62
fi
if [ $ret3 -ne 421 ];then
     exit 63
fi
if [ $ret4 -ne 200 ];then
     exit 64
fi
if [ $ret5 -ne 404 ];then
     exit 65
fi
if [ $ret6 -ne 421 ];then
     exit 66
fi
if [ $ret7 -ne 421 ];then
     exit 67
fi
if [ $ret8 -ne 421 ];then
     exit 68
fi
if [ $ret9 -ne 404 ];then
     exit 69
fi

if [ "$is_release" = "yes" ];then
    if [ -e "cicd/target/$target/gen.sh" ];then
        "cicd/target/$target/gen.sh" "$ngxver" "$triple"
        exit $?
    else
        echo no gen.sh
        exit 0
    fi
fi
