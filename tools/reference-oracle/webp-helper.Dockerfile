# Only the build stage acquires a compiler. Runtime keeps the original codec
# libraries and complete Playwright closure; never copy builder libraries.
FROM mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9 AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev
COPY scripts/webp-encode-inputs.c /build/webp-encode-inputs.c
COPY webp-helper.Dockerfile /build/webp-helper.Dockerfile
RUN --network=none gcc -std=c11 -O2 -Wall -Wextra -Werror -Wconversion -Wpedantic \
      /build/webp-encode-inputs.c /lib/x86_64-linux-gnu/libwebp.so.7 \
      -o /build/webp-encode-inputs \
    && gcc --version > /build/compiler.txt \
    && dpkg-query -W > /build/build-packages.txt \
    && sha256sum /build/webp-encode-inputs.c > /build/source.sha256

FROM mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9
COPY --from=build /build/ /opt/webp-helper/
