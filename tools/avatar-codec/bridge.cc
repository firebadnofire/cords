// Cords' narrow libjxl ABI. Each WASM instance owns one operation; no filesystem,
// threads, network, or native browser codec. The host must also reserve linear
// memory and copied/transferred buffers against its aggregate operation budget.
#include <jxl/decode.h>
#include <jxl/encode.h>
#include <jxl/cms.h>

#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <limits>

namespace {
constexpr size_t kMaxHeap = 256u * 1024u * 1024u;
constexpr uint64_t kMaxPixels = 32'000'000;
constexpr uint32_t kMaxFrames = 1000;
constexpr uint64_t kMaxCumulativePixels = 100'000'000;
constexpr uint32_t kMaxDimension = 8192;
struct alignas(std::max_align_t) Allocation { size_t size; };
size_t limit = kMaxHeap;
size_t used = 0;
size_t peak = 0;
const char* error = "";
JxlEncoder* encoder = nullptr;
JxlEncoderFrameSettings* settings = nullptr;
JxlDecoder* decoder = nullptr;
JxlBasicInfo basic{};
JxlFrameHeader frame{};
uint32_t encode_count = 0;
uint32_t decode_count = 0;
uint64_t cumulative_pixels = 0;
uint64_t cumulative_ticks = 0;
bool closed = false;
bool frame_pending = false;
constexpr size_t kMaxMetadata = 1024u * 1024u;
uint8_t box_buffer[65536];
bool box_pending = false;
size_t metadata_bytes = 0;
int encoder_status = JXL_ENC_SUCCESS;
JxlPixelFormat format{4, JXL_TYPE_UINT8, JXL_NATIVE_ENDIAN, 0};

int fail(const char* message) { error = message; return -1; }
bool count_metadata(size_t bytes) {
  if (bytes > kMaxMetadata - metadata_bytes) return false;
  metadata_bytes += bytes;
  return true;
}
bool release_box() {
  if (!box_pending) return true;
  const size_t remaining = JxlDecoderReleaseBoxBuffer(decoder);
  box_pending = false;
  return remaining <= sizeof(box_buffer) && count_metadata(sizeof(box_buffer) - remaining);
}
void* allocate(void*, size_t size) {
  if (size > limit || used > limit - size ||
      size > std::numeric_limits<size_t>::max() - sizeof(Allocation)) {
    error = "JXL allocation budget exceeded";
    return nullptr;
  }
  auto* allocation = static_cast<Allocation*>(std::malloc(sizeof(Allocation) + size));
  if (!allocation) { error = "JXL linear memory exhausted"; return nullptr; }
  allocation->size = size;
  used += size;
  if (used > peak) peak = used;
  return allocation + 1;
}
void release(void*, void* pointer) {
  if (!pointer) return;
  auto* allocation = static_cast<Allocation*>(pointer) - 1;
  used -= allocation->size;
  std::free(allocation);
}
JxlMemoryManager memory{nullptr, allocate, release};
bool dimensions(uint32_t width, uint32_t height) {
  return width && height && width <= kMaxDimension && height <= kMaxDimension &&
         static_cast<uint64_t>(width) * height <= kMaxPixels;
}
bool count_frame(uint32_t count, uint64_t pixels) {
  return count < kMaxFrames && pixels <= kMaxCumulativePixels &&
         cumulative_pixels <= kMaxCumulativePixels - pixels;
}
bool add_duration(uint32_t duration) {
  if (cumulative_ticks > std::numeric_limits<uint64_t>::max() - duration) return false;
  cumulative_ticks += duration;
  // 1000 uint32 durations * uint32 denominator fits uint64 after checking.
  const auto denominator = basic.animation.tps_denominator;
  if (!denominator || !basic.animation.tps_numerator ||
      cumulative_ticks > std::numeric_limits<uint64_t>::max() / denominator) return false;
  return cumulative_ticks * denominator <=
         uint64_t{120} * basic.animation.tps_numerator;
}
}  // namespace

extern "C" {
void* cords_alloc(size_t size) { return allocate(nullptr, size); }
void cords_free(void* pointer) { release(nullptr, pointer); }
size_t cords_used() { return used; }
size_t cords_peak() { return peak; }
const char* cords_error() { return error; }
int cords_budget(size_t bytes) {
  if (!bytes || bytes > kMaxHeap || bytes < used) return fail("Invalid JXL allocation budget");
  limit = bytes;
  return 0;
}
void cords_reset() {
  if (encoder) JxlEncoderDestroy(encoder);
  if (decoder) JxlDecoderDestroy(decoder);
  encoder = nullptr; settings = nullptr; decoder = nullptr;
  error = ""; basic = {}; frame = {};
  encode_count = decode_count = 0;
  cumulative_pixels = cumulative_ticks = 0;
  closed = frame_pending = false;
  box_pending = false; metadata_bytes = 0;
  encoder_status = JXL_ENC_SUCCESS;
  peak = used;  // External input/output allocations remain owned by the caller.
}

int cords_encode_begin(uint32_t width, uint32_t height, uint32_t bits,
                       uint32_t animated, uint32_t numerator,
                       uint32_t denominator, uint32_t loops) {
  if (encoder || decoder) return fail("Dispose the previous JXL operation first");
  if (!dimensions(width, height) || width > 1024 || height > 1024 ||
      (bits != 8 && bits != 16) || animated > 1 ||
      (animated && (!numerator || !denominator))) return fail("Invalid JXL encoding parameters");
  encoder = JxlEncoderCreate(&memory);
  if (!encoder) return fail("JXL encoder allocation failed");
  JxlEncoderInitBasicInfo(&basic);
  basic.xsize = width; basic.ysize = height;
  basic.bits_per_sample = basic.alpha_bits = bits;
  basic.num_color_channels = 3; basic.num_extra_channels = 1;
  basic.uses_original_profile = JXL_TRUE;
  basic.have_animation = animated ? JXL_TRUE : JXL_FALSE;
  basic.animation.tps_numerator = numerator;
  basic.animation.tps_denominator = denominator;
  basic.animation.num_loops = loops;
  format.data_type = bits == 16 ? JXL_TYPE_UINT16 : JXL_TYPE_UINT8;
  JxlColorEncoding color;
  JxlColorEncodingSetToSRGB(&color, JXL_FALSE);
  if (JxlEncoderSetCodestreamLevel(encoder, 10) != JXL_ENC_SUCCESS ||
      JxlEncoderSetBasicInfo(encoder, &basic) != JXL_ENC_SUCCESS ||
      JxlEncoderSetColorEncoding(encoder, &color) != JXL_ENC_SUCCESS)
    return fail("JXL basic/color configuration failed");
  settings = JxlEncoderFrameSettingsCreate(encoder, nullptr);
  if (!settings || JxlEncoderSetFrameLossless(settings, JXL_TRUE) != JXL_ENC_SUCCESS ||
      JxlEncoderSetExtraChannelDistance(settings, 0, 0) != JXL_ENC_SUCCESS ||
      JxlEncoderFrameSettingsSetOption(settings, JXL_ENC_FRAME_SETTING_KEEP_INVISIBLE, 1) != JXL_ENC_SUCCESS ||
      JxlEncoderFrameSettingsSetOption(settings, JXL_ENC_FRAME_SETTING_EFFORT, 3) != JXL_ENC_SUCCESS)
    return fail("JXL lossless frame configuration failed");
  return 0;
}
int cords_encode_frame(const void* pixels, size_t size, uint32_t duration, uint32_t last) {
  if (!encoder || closed || !pixels || last > 1) return fail("Invalid JXL encoder state");
  const uint64_t canvas = uint64_t{basic.xsize} * basic.ysize;
  const uint64_t expected = canvas * 4 * (basic.bits_per_sample / 8);
  if (size != expected || !count_frame(encode_count, canvas) ||
      (!basic.have_animation && (encode_count || duration || !last)))
    return fail("Invalid JXL frame size/count");
  if (basic.have_animation && !add_duration(duration)) return fail("JXL animation duration exceeds limit");
  JxlEncoderInitFrameHeader(&frame);
  frame.duration = duration;
  // Preserve even zero-duration frames as distinct full-canvas replacement layers.
  frame.layer_info.blend_info.blendmode = JXL_BLEND_REPLACE;
  if (JxlEncoderSetFrameHeader(settings, &frame) != JXL_ENC_SUCCESS ||
      JxlEncoderAddImageFrame(settings, &format, pixels, size) != JXL_ENC_SUCCESS)
    return fail("JXL frame encoding failed");
  ++encode_count; cumulative_pixels += canvas;
  if (last) { JxlEncoderCloseInput(encoder); closed = true; }
  return 0;
}
int cords_encode_read(void* output, size_t capacity) {
  if (!encoder || !output || !capacity || !encode_count) return fail("Invalid JXL output buffer/state");
  auto* next = static_cast<uint8_t*>(output);
  size_t remaining = capacity;
  encoder_status = JxlEncoderProcessOutput(encoder, &next, &remaining);
  if (encoder_status == JXL_ENC_ERROR) return fail("JXL output encoding failed");
  return static_cast<int>(capacity - remaining);
}
int cords_encode_status() { return encoder_status; }

int cords_decode_begin(const uint8_t* input, size_t size, uint32_t coalesce) {
  if (encoder || decoder || !input || !size || size > 50'000'000 || coalesce > 1)
    return fail("Invalid JXL decoder input/state");
  decoder = JxlDecoderCreate(&memory);
  if (!decoder) return fail("JXL decoder allocation failed");
  if (JxlDecoderSubscribeEvents(decoder, JXL_DEC_BASIC_INFO | JXL_DEC_COLOR_ENCODING |
      JXL_DEC_FRAME | JXL_DEC_FULL_IMAGE | JXL_DEC_BOX | JXL_DEC_BOX_COMPLETE) != JXL_DEC_SUCCESS ||
      JxlDecoderSetCms(decoder, *JxlGetDefaultCms()) != JXL_DEC_SUCCESS ||
      JxlDecoderSetDecompressBoxes(decoder, JXL_TRUE) != JXL_DEC_SUCCESS ||
      JxlDecoderSetCoalescing(decoder, coalesce ? JXL_TRUE : JXL_FALSE) != JXL_DEC_SUCCESS ||
      JxlDecoderSetUnpremultiplyAlpha(decoder, JXL_TRUE) != JXL_DEC_SUCCESS ||
      JxlDecoderSetInput(decoder, input, size) != JXL_DEC_SUCCESS)
    return fail("JXL decoder configuration failed");
  JxlDecoderCloseInput(decoder);
  return 0;
}
// Return 1 = basic info, 2 = frame header, 3 = pixels needed, 4 = frame complete,
// 0 = complete codestream, -1 = failure. Host supplies one bounded output buffer.
int cords_decode_next() {
  if (!decoder) return fail("JXL decoder is not open");
  for (;;) {
    const auto status = JxlDecoderProcessInput(decoder);
    if (status == JXL_DEC_BOX || status == JXL_DEC_BOX_COMPLETE ||
        status == JXL_DEC_BOX_NEED_MORE_OUTPUT || status == JXL_DEC_SUCCESS) {
      if (!release_box()) return fail("JXL decompressed metadata exceeds 1 MiB");
      if (status == JXL_DEC_BOX) {
        JxlBoxType type;
        uint64_t size = 0;
        if (JxlDecoderGetBoxType(decoder, type, JXL_TRUE) != JXL_DEC_SUCCESS ||
            JxlDecoderGetBoxSizeRaw(decoder, &size) != JXL_DEC_SUCCESS)
          return fail("Malformed JXL container box");
        if (!std::memcmp(type, "jxlc", 4) || !std::memcmp(type, "jxlp", 4)) continue;
        if (size > kMaxMetadata) return fail("JXL metadata box exceeds 1 MiB");
      }
      if (status == JXL_DEC_BOX || status == JXL_DEC_BOX_NEED_MORE_OUTPUT) {
        if (JxlDecoderSetBoxBuffer(decoder, box_buffer, sizeof(box_buffer)) != JXL_DEC_SUCCESS)
          return fail("JXL metadata output failed");
        box_pending = true;
        continue;
      }
      if (status == JXL_DEC_BOX_COMPLETE) continue;
    }
    if (status == JXL_DEC_BASIC_INFO) {
      if (JxlDecoderGetBasicInfo(decoder, &basic) != JXL_DEC_SUCCESS ||
          !dimensions(basic.xsize, basic.ysize) || basic.exponent_bits_per_sample ||
          (basic.bits_per_sample != 8 && basic.bits_per_sample != 16) ||
          basic.alpha_exponent_bits || basic.num_color_channels != 3 ||
          basic.num_extra_channels > 1)
        return fail("Unsupported or excessive JXL dimensions/sample format");
      if (basic.num_extra_channels) {
        JxlExtraChannelInfo extra;
        if (JxlDecoderGetExtraChannelInfo(decoder, 0, &extra) != JXL_DEC_SUCCESS ||
            extra.type != JXL_CHANNEL_ALPHA || extra.bits_per_sample != basic.bits_per_sample ||
            extra.exponent_bits_per_sample || extra.dim_shift)
          return fail("Unsupported JXL extra channel/alpha precision");
      }
      format.data_type = basic.bits_per_sample == 16 ? JXL_TYPE_UINT16 : JXL_TYPE_UINT8;
      return 1;
    }
    if (status == JXL_DEC_COLOR_ENCODING) {
      size_t icc_size = 0;
      if (JxlDecoderGetICCProfileSize(decoder, JXL_COLOR_PROFILE_TARGET_ORIGINAL, &icc_size) != JXL_DEC_SUCCESS ||
          !count_metadata(icc_size)) return fail("JXL color metadata exceeds 1 MiB");
      JxlColorEncoding color;
      JxlColorEncodingSetToSRGB(&color, JXL_FALSE);
      if (JxlDecoderSetOutputColorProfile(decoder, &color, nullptr, 0) != JXL_DEC_SUCCESS)
        return fail("JXL sRGB color conversion failed");
      continue;
    }
    if (status == JXL_DEC_FRAME) {
      const uint64_t canvas = uint64_t{basic.xsize} * basic.ysize;
      if (JxlDecoderGetFrameHeader(decoder, &frame) != JXL_DEC_SUCCESS ||
          !count_frame(decode_count, canvas)) return fail("JXL frame count/pixels exceed limit");
      if (basic.have_animation && !add_duration(frame.duration))
        return fail("JXL animation duration exceeds limit");
      ++decode_count; cumulative_pixels += canvas; frame_pending = true;
      return 2;
    }
    if (status == JXL_DEC_NEED_IMAGE_OUT_BUFFER) return 3;
    if (status == JXL_DEC_FULL_IMAGE) { frame_pending = false; return 4; }
    if (status == JXL_DEC_SUCCESS) return 0;
    return fail("Malformed, truncated or resource-exhausting JXL");
  }
}
uint32_t cords_info(uint32_t field) {
  switch (field) {
    case 0: return basic.xsize;
    case 1: return basic.ysize;
    case 2: return basic.bits_per_sample;
    case 3: return basic.have_animation;
    case 4: return basic.animation.tps_numerator;
    case 5: return basic.animation.tps_denominator;
    case 6: return basic.animation.num_loops;
    case 7: return frame.duration;
    case 8: return decode_count;
    case 9: return frame.is_last;
    case 10: return frame.layer_info.xsize;
    case 11: return frame.layer_info.ysize;
    case 12: return static_cast<uint32_t>(frame.layer_info.crop_x0);
    case 13: return static_cast<uint32_t>(frame.layer_info.crop_y0);
    case 14: return frame.layer_info.blend_info.blendmode;
    case 15: return frame.layer_info.blend_info.source;
    case 16: return frame.layer_info.save_as_reference;
    default: return 0;
  }
}
int cords_decode_pixels(void* pixels, size_t capacity) {
  size_t required = 0;
  if (!decoder || !frame_pending || !pixels ||
      JxlDecoderImageOutBufferSize(decoder, &format, &required) != JXL_DEC_SUCCESS ||
      required > capacity || JxlDecoderSetImageOutBuffer(decoder, &format, pixels, capacity) != JXL_DEC_SUCCESS)
    return fail("JXL pixel output exceeds supplied buffer");
  return 0;
}
}  // extern "C"
