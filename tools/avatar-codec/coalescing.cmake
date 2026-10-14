# The pinned decoder normally emits only the last layer of a displayed still.
# Expose regular zero-duration layers as well, keeping native blending/reference
# handling enabled. Generate a copy; never modify the verified upstream checkout.
set(cords_decoder "${CORDS_LIBJXL_SOURCE}/lib/jxl/decode.cc")
file(SHA256 "${cords_decoder}" cords_decoder_hash)
if(NOT cords_decoder_hash STREQUAL "c6afc0252fb0abff0dd57eec9742af87b609ba5b49692eb3d936af46d5a13e61")
  message(FATAL_ERROR "Pinned libjxl decoder changed; review zero-duration layer patch")
endif()
file(READ "${cords_decoder}" cords_decoder_text)
string(REPLACE
  "(!dec->coalescing &&\n           dec->frame_header->frame_type == FrameType::kRegularFrame)"
  "(dec->frame_header->frame_type == FrameType::kRegularFrame)"
  cords_patched_decoder "${cords_decoder_text}")
if(cords_patched_decoder STREQUAL cords_decoder_text)
  message(FATAL_ERROR "Zero-duration layer patch context was not found")
endif()
set(cords_generated_decoder "${CMAKE_CURRENT_BINARY_DIR}/cords-decode.cc")
set(cords_existing_decoder "")
if(EXISTS "${cords_generated_decoder}")
  file(READ "${cords_generated_decoder}" cords_existing_decoder)
endif()
if(NOT cords_existing_decoder STREQUAL cords_patched_decoder)
  file(WRITE "${cords_generated_decoder}" "${cords_patched_decoder}")
endif()
get_target_property(cords_decoder_sources jxl_dec-obj SOURCES)
list(FIND cords_decoder_sources "jxl/decode.cc" cords_decoder_index)
if(cords_decoder_index EQUAL -1)
  message(FATAL_ERROR "Pinned libjxl decoder target layout changed")
endif()
list(REMOVE_AT cords_decoder_sources ${cords_decoder_index})
list(APPEND cords_decoder_sources "${cords_generated_decoder}")
set_property(TARGET jxl_dec-obj PROPERTY SOURCES ${cords_decoder_sources})
