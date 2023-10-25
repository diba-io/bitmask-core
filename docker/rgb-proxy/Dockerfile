FROM node:18-bullseye
ARG BUILDER_SRC=/srv/src/
ARG SOURCE_CODE=https://github.com/grunch/rgb-proxy-server.git
ARG VERSION=main

RUN apt-get -y update \
    && apt-get -y install tini git \
    && apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

WORKDIR $BUILDER_SRC
RUN git clone $SOURCE_CODE $BUILDER_SRC
RUN git checkout $VERSION

RUN npm install
RUN npm run build

ENV NODE_ENV="production"

EXPOSE 3000/tcp
VOLUME ["/root/.npm", "/root/.rgb-proxy-server"]

CMD ["tini", "--", "npm", "run", "start"]
