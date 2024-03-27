FROM amd64/debian:12
#ver 1 (MUST INCREASE WHEN CHANGED!!! and compile.yml!!!)
RUN apt-get -y update
RUN apt-get -y install curl build-essential libclang-dev
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
