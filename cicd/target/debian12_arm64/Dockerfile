FROM arm64v8/debian:12

RUN apt-get -y update
RUN apt-get -y install curl build-essential libclang-dev
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
