FROM rust:1.97.1-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends build-essential pkg-config libssl-dev ca-certificates git python3 python3-pip python3-venv clang lld llvm ninja-build libgl1 libegl1 libfontconfig1 libdbus-1-3 libx11-6 libx11-xcb1 libxcb-cursor0 libxcb-icccm4 libxcb-image0 libxcb-keysyms1 libxcb-render-util0 libxcb-xinerama0 libxkbcommon-x11-0 && rm -rf /var/lib/apt/lists/*
RUN python3 -m venv /opt/cords-tools && /opt/cords-tools/bin/pip install --no-cache-dir cmake==3.31.6 aqtinstall==3.3.0
RUN rustup target add --toolchain 1.97.1 x86_64-pc-windows-msvc && cargo install cargo-xwin --version 0.23.1 --locked
RUN /opt/cords-tools/bin/aqt install-qt linux desktop 6.10.3 linux_gcc_64 -O /opt/qt --archives icu qtbase qtdeclarative qtsvg qttools qttranslations qtwayland && /opt/cords-tools/bin/aqt install-qt windows desktop 6.10.3 win64_msvc2022_64 -O /opt/qt --archives qtbase qtdeclarative qtsvg qttools qttranslations
ENV PATH=/opt/cords-tools/bin:/usr/local/cargo/bin:$PATH
# Debian packages Clang's MSVC driver under the clang binary only.
RUN apt-get update && apt-get install -y --no-install-recommends curl gnupg && \
    curl -fsSL https://apt.llvm.org/llvm-snapshot.gpg.key | gpg --dearmor -o /usr/share/keyrings/llvm-snapshot.gpg && \
    printf '%s\n' 'deb [signed-by=/usr/share/keyrings/llvm-snapshot.gpg] https://apt.llvm.org/bookworm/ llvm-toolchain-bookworm-19 main' > /etc/apt/sources.list.d/llvm19.list && \
    apt-get update && apt-get install -y --no-install-recommends clang-19 lld-19 llvm-19 && \
    rm -rf /var/lib/apt/lists/* && \
    ln -s /usr/bin/clang-19 /usr/local/bin/clang-cl && \
    ln -s /usr/bin/lld-link-19 /usr/local/bin/lld-link && \
    ln -s /usr/bin/llvm-lib-19 /usr/local/bin/llvm-lib && \
    ln -s /usr/bin/llvm-rc-19 /usr/local/bin/llvm-rc && \
    ln -s /usr/bin/llvm-mt-19 /usr/local/bin/llvm-mt && \
    clang-cl --version | grep -q 'version 19\.' 
