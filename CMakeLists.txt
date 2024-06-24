cmake_minimum_required(VERSION 3.23)

project(vorg CXX)

include(CTest)

# Enable coverage on debug builds
set(CMAKE_CXX_FLAGS_DEBUG "${CMAKE_CXX_FLAGS_DEBUG} --coverage")

add_subdirectory(src)
add_subdirectory(tests)
