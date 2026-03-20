FROM alpine:3.20
# Package versions pinned to Alpine 3.20. Update together with the FROM base tag.
RUN apk add --no-cache ca-certificates=20250911-r0
ARG BINARY=crap-cms-linux-x86_64
COPY ${BINARY} /usr/local/bin/crap-cms
RUN chmod +x /usr/local/bin/crap-cms
COPY example/ /example/
RUN crap-cms -C /example migrate up
VOLUME ["/config"]
EXPOSE 3000 50051
ENTRYPOINT ["crap-cms"]
CMD ["-C", "/config", "serve"]
