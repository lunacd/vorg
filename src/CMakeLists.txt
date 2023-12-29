add_executable(vorg vorg.cpp)

add_subdirectory(db)
add_subdirectory(utils)

target_include_directories(vorg PRIVATE ./db ./utils)
target_link_libraries(vorg PUBLIC vorgDb vorgUtils)
