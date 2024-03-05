add_executable(vorg vorg.cpp)

target_include_directories(vorg PRIVATE .)

find_package(Boost REQUIRED COMPONENTS program_options)
target_link_libraries(vorg PRIVATE Boost::program_options)

add_library(libVorg OBJECT
    vorg_server.cpp
    vorg_errors.cpp
    vorg_db.cpp
)

target_sources(libVorg PUBLIC
    FILE_SET vorg_headers
    TYPE HEADERS
    FILES
    vorg_server.h
    vorg_errors.h
    vorg_db.h
)

target_include_directories(libVorg PRIVATE .)
target_link_libraries(vorg PRIVATE libVorg)

find_package(SQLiteCpp CONFIG REQUIRED)
target_link_libraries(libVorg PRIVATE SQLiteCpp)

find_package(inja CONFIG REQUIRED)
target_link_libraries(libVorg PRIVATE pantor::inja)

find_package(Drogon CONFIG REQUIRED)
target_link_libraries(libVorg PRIVATE Drogon::Drogon)
