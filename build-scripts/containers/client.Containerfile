FROM rust:1.97.1-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends build-essential pkg-config libssl-dev ca-certificates git python3 python3-pip python3-venv libgl1-mesa-dev libegl1 libx11-xcb1 libxcb-cursor0 libxcb-icccm4 libxcb-image0 libxcb-keysyms1 libxcb-render-util0 libxcb-xinerama0 libxkbcommon-x11-0 libfontconfig1 libdbus-1-3 libxrender1 libxi6 libxext6 libx11-6 libopengl0 libglx0 libvulkan1 libwayland-client0 libwayland-egl1 && rm -rf /var/lib/apt/lists/*
RUN python3 -m venv /opt/cords-tools && /opt/cords-tools/bin/pip install --no-cache-dir cmake==3.31.6 aqtinstall==3.3.0
RUN /opt/cords-tools/bin/aqt install-qt linux desktop 6.10.3 linux_gcc_64 -O /opt/qt --archives icu qtbase qtdeclarative qtsvg qttools qttranslations qtwayland
ENV PATH=/opt/cords-tools/bin:/usr/local/cargo/bin:$PATH
ENV CORDS_QT_PREFIX=/opt/qt/6.10.3/gcc_64
