cmake_minimum_required(VERSION 3.31)
project(cords_avatar_codec LANGUAGES C CXX)

if(NOT EMSCRIPTEN)
  message(FATAL_ERROR "Use the pinned Emscripten toolchain via build.py")
endif()
if(NOT DEFINED CORDS_LIBJXL_SOURCE)
  message(FATAL_ERROR "CORDS_LIBJXL_SOURCE must name the verified libjxl checkout")
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
set(JPEGXL_ENABLE_TOOLS OFF CACHE BOOL "" FORCE)
set(JPEGXL_ENABLE_DEVTOOLS OFF CACHE BOOL "" FORCE)
set(JPEGXL_ENABLE_BENCHMARK OFF CACHE BOOL "" FORCE)
set(JPEGXL_ENABLE_EXAMPLES OFF CACHE BOOL "" FORCE)
set(JPEGXL_ENABLE_MANPAGES OFF CACHE BOOL "" FORCE)
set(JPEGXL_ENABLE_DOXYGEN OFF CACHE BOOL "" FORCE)
set(JPEGXL_ENABLE_JNI OFF CACHE BOOL "" FORCE)
set(JPEGXL_ENABLE_SJPEG OFF CACHE BOOL "" FORCE)
set(JPEGXL_ENABLE_OPENEXR OFF CACHE BOOL "" FORCE)
set(JPEGXL_ENABLE_TCMALLOC OFF CACHE BOOL "" FORCE)
set(JPEGXL_ENABLE_WASM_THREADS OFF CACHE BOOL "" FORCE)
set(JPEGXL_ENABLE_TRANSCODE_JPEG OFF CACHE BOOL "" FORCE)
set(JPEGXL_ENABLE_SKCMS ON CACHE BOOL "" FORCE)
add_subdirectory("${CORDS_LIBJXL_SOURCE}" libjxl EXCLUDE_FROM_ALL)
include("${CMAKE_CURRENT_SOURCE_DIR}/coalescing.cmake")

add_executable(cords-fixtures EXCLUDE_FROM_ALL fixtures.cc)
target_link_libraries(cords-fixtures PRIVATE jxl)
target_compile_options(cords-fixtures PRIVATE -Wall -Wextra -Werror)
set_target_properties(cords-fixtures PROPERTIES SUFFIX ".mjs")
target_link_options(cords-fixtures PRIVATE --no-entry "-sMODULARIZE=1" "-sEXPORT_ES6=1"
  "-sENVIRONMENT=node" "-sFILESYSTEM=0" "-sALLOW_MEMORY_GROWTH=1"
  "-sMAXIMUM_MEMORY=67108864" "-sEXPORTED_RUNTIME_METHODS=['HEAPU8']"
  "-sEXPORTED_FUNCTIONS=['_fixture_build','_fixture_data','_fixture_error']")

add_executable(cords-jxl bridge.cc)
target_compile_options(cords-jxl PRIVATE -Wall -Wextra -Werror)
target_link_libraries(cords-jxl PRIVATE jxl jxl_cms)
set_target_properties(cords-jxl PROPERTIES SUFFIX ".mjs")
target_link_options(cords-jxl PRIVATE
  --no-entry
  "-sMODULARIZE=1"
  "-sEXPORT_ES6=1"
  "-sENVIRONMENT=web,worker,node"
  "-sFILESYSTEM=0"
  "-sALLOW_MEMORY_GROWTH=1"
  "-sINITIAL_MEMORY=33554432"
  "-sIMPORTED_MEMORY=1"
  "-sMAXIMUM_MEMORY=268435456"
  "-sABORTING_MALLOC=0"
  "-sSTACK_SIZE=1048576"
  "-sEXPORTED_RUNTIME_METHODS=['HEAPU8']"
  "-sEXPORTED_FUNCTIONS=['_cords_alloc','_cords_free','_cords_reset','_cords_budget','_cords_used','_cords_peak','_cords_error','_cords_encode_begin','_cords_encode_frame','_cords_encode_read','_cords_encode_status','_cords_decode_begin','_cords_decode_next','_cords_info','_cords_decode_pixels']"
)
