#Buildere
FROM rust:1.69-slim-buster AS builder
ARG BUILDER_DIR=/srv/bitmask
ARG BUILDER_SRC=/opt/src/bitmask

RUN apt-get update -y && \
  apt-get install -y pkg-config make g++ libssl-dev

WORKDIR $BUILDER_DIR
WORKDIR $BUILDER_SRC
COPY . .

RUN cargo install --locked --features server --path . --root ${BUILDER_DIR}

# Set the working directory inside the container
#WORKDIR /app

# Runtime
FROM rust:1.69-slim-buster AS runtime

ARG BUILDER_DIR=/srv/bitmask
ARG BIN_DIR=/usr/local/bin
ARG DATA_DIR=/data/bitmaskd/carbonado/
ARG USER=bitmask

RUN apt-get update -y && apt-get install -y iputils-ping telnet \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

RUN adduser --home "${DATA_DIR}" --shell /bin/bash --disabled-login \
            --gecos "${USER} user" ${USER}

COPY --from=builder --chown=${USER}:${USER} \
     "${BUILDER_DIR}/bin/" "${BIN_DIR}"

USER ${USER}
VOLUME ${DATA_DIR}
EXPOSE 7070

# Set the data directory for data persistence
WORKDIR ${BIN_DIR}

ENTRYPOINT ["bitmaskd"]
