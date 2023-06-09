# Builder
FROM rust:slim AS builder
ARG BUILDER_DIR=/srv/bitmask
ARG BUILDER_SRC=/opt/src/bitmask

RUN apt-get update -y && \
  apt-get install -y pkg-config make g++ libssl-dev && \
  rustup target add x86_64-unknown-linux-gnu

WORKDIR $BUILDER_DIR
WORKDIR $BUILDER_SRC
COPY . .

RUN cargo install --locked --all-features --path . --root ${BUILDER_DIR} --target x86_64-unknown-linux-gnu

# Runtime
FROM rust:slim AS runtime

ARG BUILDER_DIR=/srv/bitmask
ARG BIN_DIR=/usr/local/bin
ARG DATA_DIR=/tmp/bitmaskd/carbonado/
ARG USER=bitmask

RUN apt-get update -y \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

RUN adduser --home "${DATA_DIR}" --shell /bin/bash --disabled-login \
            --gecos "${USER} user" ${USER}

COPY --from=builder --chown=${USER}:${USER} \
     "${BUILDER_DIR}/bin/" "${BIN_DIR}"

USER ${USER}
VOLUME ${DATA_DIR}
EXPOSE 7070

WORKDIR ${BIN_DIR}

ENTRYPOINT ["bitmaskd"]
