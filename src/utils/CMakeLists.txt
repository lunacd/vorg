add_library(vorgUtils OBJECT vorg_utils_error.cpp)

target_sources(vorgUtils PUBLIC
    FILE_SET utils_headers
    TYPE HEADERS
    FILES
    vorg_utils_error.h
)

target_include_directories(vorgUtils PRIVATE .)
