FROM amd64/debian:11.9-slim
#ver 2 (MUST INCREASE WHEN CHANGED!!! and compile.yml!!!)
RUN apt-get -y update
RUN apt-get -y install curl build-essential libclang-dev libssl-dev pkg-config
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH $PATH:/root/.cargo/bin
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall -y cargo-deb
