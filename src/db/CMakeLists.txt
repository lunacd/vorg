add_library(vorgDb OBJECT
    vorg_db.cpp
)

target_sources(vorgDb PRIVATE 
    FILE_SET db_headers
    TYPE HEADERS
    FILES
    vorg_db.h
)

find_package(SQLiteCpp CONFIG REQUIRED)
target_link_libraries(vorgDb PUBLIC SQLiteCpp)

target_include_directories(vorgDb PRIVATE . ../utils)
