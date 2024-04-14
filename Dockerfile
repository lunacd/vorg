### STAGE 0: build vorg backend ###
FROM debian:bookworm

# Install dependencies
RUN --mount=target=/var/cache/apt,type=cache,sharing=locked \
    apt-get update && \
    apt-get install -y \
        curl zip unzip tar \
        git \
        pkg-config \
        cmake \
        ninja-build \
        g++

# Set up vcpkg
WORKDIR /
RUN git clone https://github.com/microsoft/vcpkg.git --depth 1
RUN cd vcpkg && ./bootstrap-vcpkg.sh

WORKDIR /workarea

# Install dependencies
COPY vcpkg.json vcpkg.json
RUN /vcpkg/vcpkg install --triplet x64-linux-release

# Copy source
COPY CMakeLists.txt CMakeLists.txt
COPY tests tests
COPY src src

# Build vorg
RUN mkdir build
RUN cmake -G Ninja -B build -S . \
    -DCMAKE_TOOLCHAIN_FILE=/vcpkg/scripts/buildsystems/vcpkg.cmake \
    -DVCPKG_TARGET_TRIPLET=x64-linux-release \
    -DCMAKE_BUILD_TYPE=RelWithDebInfo
RUN cmake --build build
RUN cmake --install build --prefix=/vorg

### Stage 1: Build vorg frontend ###

### Stage 2: Final image ###
# Copy over binary
FROM debian:bookworm
COPY --from=0 /vorg/bin/vorg /bin/vorg
