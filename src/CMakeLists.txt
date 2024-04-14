add_executable(vorg vorg.cpp)

target_include_directories(vorg PRIVATE .)

find_package(Boost REQUIRED COMPONENTS program_options)
target_link_libraries(vorg PRIVATE Boost::boost Boost::program_options)

add_library(libVorg OBJECT
    vorg_errors.cpp
    vorg_db.cpp
    vorg_server.cpp
    models/vorg_collection.cpp
    models/vorg_item.cpp
)

target_sources(libVorg PUBLIC
    FILE_SET vorg_headers
    TYPE HEADERS
    FILES
    vorg_errors.h
    vorg_db.h
    vorg_server.h
    models/vorg_collection.h
    models/vorg_item.h
)

target_include_directories(libVorg PRIVATE . ./server)
target_link_libraries(vorg PRIVATE libVorg)

find_package(SQLiteCpp CONFIG REQUIRED)
target_link_libraries(libVorg PRIVATE SQLiteCpp)

find_package(Crow CONFIG REQUIRED)
target_link_libraries(libVorg PRIVATE Crow::Crow)

find_package(Boost REQUIRED COMPONENTS log)
target_link_libraries(libVorg PRIVATE Boost::log)

install(TARGETS vorg)
