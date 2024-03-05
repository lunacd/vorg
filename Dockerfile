### STAGE 0: build vorg backend ###
FROM ubuntu:23.10

# Install dependencies
RUN apt-get update && \
    apt-get install -y \
        curl zip unzip tar \
        git \
        pkg-config \
        cmake \
        ninja-build \
        g++

# Set up vcpkg
WORKDIR /
RUN git clone https://github.com/microsoft/vcpkg.git
RUN cd vcpkg && ./bootstrap-vcpkg.sh

WORKDIR /workarea

# Install vcpkg dependencies
COPY vcpkg.json vcpkg.json
RUN /vcpkg/vcpkg install

# Copy source
COPY CMakeLists.txt CMakeLists.txt
COPY tests tests
COPY src src

# Build vorg
RUN mkdir build
RUN cmake -G Ninja -B build -S . -DCMAKE_TOOLCHAIN_FILE=/vcpkg/scripts/buildsystems/vcpkg.cmake
RUN cmake --build build

### Stage 1: Build vorg frontend ###

### Stage 2: Final image ###
# Copy over binary
FROM ubuntu:23.10
COPY --from=0 /workarea/build/src/vorg /bin/vorg

# Copy configuration file
COPY runtime runtime

CMD /bin/vorg server
