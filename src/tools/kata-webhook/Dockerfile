# Build binary of kata webhook
# FROM golang:1.16-alpine AS build-stage
# RUN apk --update --no-cache add make

# kata-webhook needs src/runtime, we will copy all kata-containers source
# build context will be /go/src/github.com/kata-containers/kata-containers
# WORKDIR /go/src/github.com/kata-containers/kata-containers
# COPY . .

# WORKDIR /go/src/github.com/kata-containers/kata-containers/src/tools/kata-webhook

# RUN make build

# The work image.
FROM alpine:3.15.2
RUN apk --no-cache add ca-certificates
# COPY --from=build-stage /go/src/github.com/kata-containers/kata-containers/src/tools/kata-webhook/bin/kata-webhook /usr/local/bin/kata-webhook
COPY bin/kata-webhook /usr/local/bin/kata-webhook

ENTRYPOINT ["/usr/local/bin/kata-webhook"]
