FROM rust:latest as cargo-build

WORKDIR /usr/src/nmmdg

COPY Cargo.toml Cargo.lock ./

RUN mkdir src/

RUN echo "#[cfg(test)] fn main() {println!(\"if you see this, the build broke\")}" > src/lib.rs

RUN cargo build --release

RUN rm -f target/release/deps/no_more_mr_dice_guy* src/lib.rs

COPY . .

RUN touch src/lib.rs

RUN cargo build --release --offline

FROM ubuntu:rolling

LABEL org.opencontainers.image.source=https://github.com/TransLunarInjection/no-more-mr-dice-guy

RUN addgroup --gid 1000 nmmdg && \
    adduser --disabled-login --shell /bin/sh --uid 1000 --ingroup nmmdg nmmdg && \
    apt-get update && apt-get install libssl1.1 libcurl4 -y && rm -rf /var/lib/apt/lists/*

WORKDIR /home/nmmdg/

COPY --from=cargo-build /usr/src/nmmdg/target/release/no_more_mr_dice_guy nmmdg

RUN chown nmmdg:nmmdg nmmdg

USER nmmdg

VOLUME /home/nmmdg/store/

ENV DISCORD_TOKEN=

CMD ["./nmmdg"]
