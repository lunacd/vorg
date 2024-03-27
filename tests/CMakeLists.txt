add_executable(vorgTests db.t.cpp)

find_package(GTest CONFIG REQUIRED)
find_package(Boost REQUIRED)
target_link_libraries(vorgTests PRIVATE
    GTest::gmock_main
    GTest::gtest
    GTest::gmock
)

# Link against libraries to test
target_link_libraries(vorgTests PRIVATE
    libVorg
)

target_include_directories(vorgTests PRIVATE ${Boost_INCLUDE_DIRS})
target_include_directories(vorgTests PRIVATE ../src/db ../src/utils)

include(GoogleTest)
gtest_discover_tests(vorgTests)
