FROM ubuntu:23.10

# Install dependencies
RUN apt-get update && \
    apt-get install -y \
        curl zip unzip tar \
        git \
        pkg-config \
        cmake \
        g++

# Set up vcpkg
WORKDIR /
RUN git clone https://github.com/microsoft/vcpkg.git
RUN cd vcpkg && ./bootstrap-vcpkg.sh

# Install vcpkg dependencies
RUN /vcpkg/vcpkg install \
    sqlite3[fts5] \
    sqlitecpp \
    gtest \
    boost-uuid

# Copy source
WORKDIR /workarea
COPY CMakeLists.txt CMakeLists.txt
COPY tests tests
COPY src src

# Build vorg
RUN mkdir build
RUN cmake -B build -S . -DCMAKE_TOOLCHAIN_FILE=/vcpkg/scripts/buildsystems/vcpkg.cmake
RUN cmake --build build

# Copy over binary
FROM ubuntu:23.04
COPY --from=0 /workarea/build/src/vorg /bin/vorg
