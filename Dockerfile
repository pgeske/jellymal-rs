FROM rust:1.71 as builder

WORKDIR /app

# download the anime mapping files
RUN wget https://raw.githubusercontent.com/Anime-Lists/anime-lists/master/anime-list-master.xml
RUN wget https://raw.githubusercontent.com/Fribb/anime-lists/master/anime-list-full.json

# build dependencies
COPY ./Cargo.toml ./Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build

# build the source code
COPY ./src ./src
RUN cargo build --release

# set environment
ENV RUST_BACKTRACE=1
ENV RUST_LOG=info

COPY ./entry-point.sh ./
CMD /app/entry-point.sh
