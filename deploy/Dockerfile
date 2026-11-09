ARG CEF_LIBS="libglib2.0-0t64 libnspr4 libnss3 libatk1.0-0t64 libatk-bridge2.0-0t64 libatspi2.0-0t64 \
    libdbus-1-3 libcups2t64 libx11-6 libxcomposite1 libxdamage1 libxext6 libxfixes3 libxrandr2 libgbm1 \
    libexpat1 libxcb1 libxkbcommon0 libcairo2 libpango-1.0-0 libudev1 libasound2t64"

FROM rust:1-trixie AS build
SHELL ["/bin/bash", "-o", "pipefail", "-c"]
WORKDIR /src
COPY . .
ARG CEF_LIBS
RUN apt-get update && apt-get install -y --no-install-recommends $CEF_LIBS \
    python3 cmake ninja-build pkg-config libssl-dev libdbus-1-dev libasound2-dev \
    && rm -rf /var/lib/apt/lists/*
RUN export SUPERFIK_RELEASE=1 \
    && . scripts/linux-engine.sh \
    && prepare_linux_engine "$(uname -m)" \
    && cargo build --release --locked -p superfik-server -p superfik-native --bin superfik-server --bin superfik-native-worker --bin superfik-session-migrate \
    && mkdir -p /out/licenses \
    && cp LICENSE NOTICE "$CEF_PATH/CREDITS.html" /out/licenses/ \
    && cargo run -q --release -p xtask -- notices /out/licenses/THIRD_PARTY_NOTICES.md
RUN mkdir -p /out/locales \
    && cd target/release \
    && cp superfik-server libcef.so libvk_swiftshader.so libvulkan.so.1 vk_swiftshader_icd.json icudtl.dat \
        v8_context_snapshot.bin resources.pak chrome_100_percent.pak chrome_200_percent.pak /out/ \
    && cp locales/*.pak /out/locales/ \
    && cp superfik-native-worker superfik-session-migrate /out/ \
    && strip --strip-unneeded /out/superfik-server /out/libcef.so /out/libvk_swiftshader.so

FROM debian:trixie-slim
ARG CEF_LIBS
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git sudo nodejs npm \
        $CEF_LIBS libvulkan1 mesa-vulkan-drivers fonts-noto-core \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system superfik-runs \
    && useradd --system --home /var/lib/superfik --gid superfik-runs superfik \
    && useradd --system --create-home --home /home/superfik-runner --gid superfik-runs superfik-runner \
    && mkdir -p /var/lib/superfik /var/lib/superfik-runs \
    && chown superfik:superfik-runs /var/lib/superfik /var/lib/superfik-runs \
    && chmod 0700 /var/lib/superfik \
    && chmod 2770 /var/lib/superfik-runs \
    && echo 'superfik ALL=(superfik-runner:superfik-runs) NOPASSWD: /usr/local/lib/superfik-server/superfik-native-worker' > /etc/sudoers.d/superfik-runner \
    && chmod 0440 /etc/sudoers.d/superfik-runner
COPY deploy/native-as-runner /usr/local/bin/native-as-runner
RUN chmod 0755 /usr/local/bin/native-as-runner
COPY --from=build /out /usr/local/lib/superfik-server
RUN ln -s /usr/local/lib/superfik-server/superfik-server /usr/local/bin/superfik-server \
    && ln -s /usr/local/lib/superfik-server/superfik-native-worker /usr/local/bin/superfik-native-worker \
    && ln -s /usr/local/lib/superfik-server/superfik-session-migrate /usr/local/bin/superfik-session-migrate
USER superfik
ENV SUPERFIK_DATA=/var/lib/superfik XDG_CONFIG_HOME=/var/lib/superfik \
    SUPERFIK_NATIVE_WORKER=/usr/local/bin/native-as-runner \
    SUPERFIK_CHECKOUTS=/var/lib/superfik-runs
VOLUME /var/lib/superfik
VOLUME /var/lib/superfik-runs
EXPOSE 7420
ENTRYPOINT ["/bin/sh", "-c", "umask 002 && exec /usr/local/bin/superfik-server \"$@\"", "superfik-server"]
