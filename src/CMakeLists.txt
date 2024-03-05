add_executable(vorg vorg.cpp)

add_subdirectory(db)
add_subdirectory(utils)
add_subdirectory(server)

target_include_directories(vorg PRIVATE .)
target_link_libraries(vorg PUBLIC vorgDb vorgUtils vorgServer)

find_package(Boost REQUIRED COMPONENTS program_options)
target_link_libraries(vorg PRIVATE Boost::program_options)
