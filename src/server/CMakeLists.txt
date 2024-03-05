add_library(vorgServer OBJECT vorg_server.cpp)

target_sources(vorgServer PUBLIC
    FILE_SET utils_headers
    TYPE HEADERS
    FILES
    vorg_server.h
)

find_package(inja CONFIG REQUIRED)
target_link_libraries(vorgServer PRIVATE pantor::inja)

find_package(Drogon CONFIG REQUIRED)
target_link_libraries(vorgServer PRIVATE Drogon::Drogon)

target_include_directories(vorgServer PRIVATE .)
