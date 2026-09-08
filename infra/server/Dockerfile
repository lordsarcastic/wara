FROM alpine:3.21

RUN apk add --no-cache bash docker-cli docker-cli-compose openssh-server shadow

RUN addgroup -S deploy \
    && adduser -S -G deploy -h /home/deploy -s /bin/sh deploy \
    && passwd -d deploy

COPY entrypoint.sh /usr/local/bin/wara-docker-server-entrypoint
RUN chmod +x /usr/local/bin/wara-docker-server-entrypoint

EXPOSE 22

ENTRYPOINT ["wara-docker-server-entrypoint"]
