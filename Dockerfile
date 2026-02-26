FROM gcr.io/distroless/static-debian13:nonroot AS typst-webservice
ARG version=dev

COPY --chown=root:root --chmod=755 ./typst-webservice-linux-x64 ./typst-webservice

EXPOSE 3000
ENV VERSION=${version}
ENTRYPOINT ["./typst-webservice"]
