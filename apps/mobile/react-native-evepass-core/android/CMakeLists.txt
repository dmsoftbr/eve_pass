cmake_minimum_required(VERSION 3.4.1)
project(EvepassCore)

set (CMAKE_VERBOSE_MAKEFILE ON)

# Rust workspace root: …/apps/mobile/react-native-evepass-core/android → repo root.
get_filename_component(REPO_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../../../.." ABSOLUTE)

# Map the Android ABI to the Rust target triple of the prebuilt static lib
# (built by scripts/build-android.sh into target/<triple>/release/).
if(ANDROID_ABI STREQUAL "arm64-v8a")
  set(RUST_TRIPLE "aarch64-linux-android")
elseif(ANDROID_ABI STREQUAL "x86_64")
  set(RUST_TRIPLE "x86_64-linux-android")
elseif(ANDROID_ABI STREQUAL "armeabi-v7a")
  set(RUST_TRIPLE "armv7-linux-androideabi")
elseif(ANDROID_ABI STREQUAL "x86")
  set(RUST_TRIPLE "i686-linux-android")
endif()

set(RUST_LIB "${REPO_ROOT}/target/${RUST_TRIPLE}/release/libevepass_core.a")

add_library(evepass_core_rust STATIC IMPORTED)
set_target_properties(evepass_core_rust PROPERTIES IMPORTED_LOCATION "${RUST_LIB}")

# UBRN runtime headers (UniffiJsiTypes.h, UniffiCallInvoker.h, Bridging.h, …).
set(UBRN_INCLUDES "${CMAKE_CURRENT_SOURCE_DIR}/../node_modules/uniffi-bindgen-react-native/cpp/includes")

add_library(
    react-native-evepass-core    STATIC
    ../cpp/EvepassCoreImpl.cpp
    ../cpp/react-native-evepass-core.cpp
    ../cpp/generated/evepass_core.cpp
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
    ../cpp/generated
    ${UBRN_INCLUDES}
)

target_link_libraries(
    react-native-evepass-core    jsi
    reactnative
    react_codegen_EvepassCoreSpec
    evepass_core_rust
    log
    android
)
