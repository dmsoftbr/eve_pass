cmake_minimum_required(VERSION 3.4.1)
project(EvepassCore)

set (CMAKE_VERBOSE_MAKEFILE ON)

add_library(
    react-native-evepass-core    STATIC
    ../cpp/EvepassCoreImpl.cpp
)

set_target_properties(
    react-native-evepass-core PROPERTIES
    CXX_STANDARD 20
    CXX_STANDARD_REQUIRED ON
    CXX_EXTENSIONS OFF
)

target_include_directories(
    react-native-evepass-core    PUBLIC
    ../cpp
)

target_link_libraries(
    react-native-evepass-core    jsi
    reactnative
    react_codegen_EvepassCoreSpec
)
