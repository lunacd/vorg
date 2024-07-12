add_executable(vorgExe vorg.cpp)
set_property(TARGET vorgExe PROPERTY OUTPUT_NAME vorg)

find_package(Boost REQUIRED COMPONENTS program_options)
target_link_libraries(vorgExe PRIVATE Boost::boost Boost::program_options)

add_library(vorg
        vorg_errors.cpp
        vorg_db.cpp
        vorg_server.cpp
        vorg_server_base.cpp
        models/vorg_collection.cpp
        models/vorg_item.cpp
)

# No deprecated functionalities from boost::asio
target_compile_definitions(vorg PRIVATE BOOST_ASIO_NO_DEPRECATED)

target_sources(
        vorg
        PUBLIC FILE_SET
        vorg_headers
        TYPE
        HEADERS
        FILES
        vorg_errors.hpp
        vorg_db.hpp
        vorg_server.hpp
        vorg_server_base.hpp
        models/vorg_collection.hpp
        models/vorg_item.hpp
)

target_include_directories(vorg PUBLIC .)
target_link_libraries(vorgExe PRIVATE vorg)

find_package(SQLiteCpp REQUIRED)
target_link_libraries(vorg PRIVATE SQLiteCpp)

find_package(Boost REQUIRED COMPONENTS log system json)
target_link_libraries(vorg PRIVATE Boost::log Boost::system Boost::json)

install(TARGETS vorg)
