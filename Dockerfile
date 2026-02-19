FROM ubuntu:24.04 AS final-base
RUN apt-get update && apt-get install adduser -y && apt-get upgrade -y

# create a non root user to run the binary
ARG user=nonroot
ARG group=nonroot
ARG uid=2000
ARG gid=2000
RUN addgroup --gid ${gid} ${group} && adduser --uid ${uid} --gid ${gid} --system --disabled-login --disabled-password ${user}

WORKDIR /home/${user}
USER $user

FROM final-base AS typst-webservice
ARG version=dev

COPY --chown=nonroot:nonroot --chmod=755 ./typst-webservice-linux-x64 ./typst-webservice

EXPOSE 3000
ENV VERSION=${version}
ENTRYPOINT ["./typst-webservice"]
CMD [ "0.0.0.0:8080" ]
