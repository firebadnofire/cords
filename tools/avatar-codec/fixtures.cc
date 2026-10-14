// Validation-only libjxl fixture generator. Never linked into the client module.
#include <jxl/encode.h>
#include <cstdint>
#include <cstdio>
#include <memory>

namespace {
uint8_t output[1024 * 1024];
const char* error = "";
char detail[128];
}
extern "C" {
const uint8_t* fixture_data() { return output; }
const char* fixture_error() { return error; }
int fixture_build(uint32_t bits, uint32_t scenario) {
  error = "";
  if (bits != 8 && bits != 16) { error = "Invalid fixture precision"; return -1; }
  if (scenario > 5) { error = "Invalid fixture scenario"; return -1; }
  std::unique_ptr<JxlEncoder, decltype(&JxlEncoderDestroy)> encoder(
      JxlEncoderCreate(nullptr), JxlEncoderDestroy);
  if (!encoder) { error = "Fixture encoder allocation failed"; return -1; }
  JxlBasicInfo basic;
  JxlEncoderInitBasicInfo(&basic);
  basic.xsize = 2; basic.ysize = 1;
  basic.bits_per_sample = basic.alpha_bits = bits;
  basic.num_color_channels = 3; basic.num_extra_channels = 1;
  basic.uses_original_profile = JXL_TRUE;
  basic.orientation = scenario == 1 ? JXL_ORIENT_ROTATE_90_CW : JXL_ORIENT_IDENTITY;
  basic.have_animation = JXL_TRUE;
  basic.animation.tps_numerator = 420;
  basic.animation.tps_denominator = 1;
  basic.animation.num_loops = 3;
  JxlColorEncoding color;
  JxlColorEncodingSetToSRGB(&color, JXL_FALSE);
  if (scenario == 2) color.transfer_function = JXL_TRANSFER_FUNCTION_LINEAR;
  if (JxlEncoderSetCodestreamLevel(encoder.get(), 10) != JXL_ENC_SUCCESS ||
      JxlEncoderSetBasicInfo(encoder.get(), &basic) != JXL_ENC_SUCCESS ||
      JxlEncoderSetColorEncoding(encoder.get(), &color) != JXL_ENC_SUCCESS) {
    error = "Fixture basic info failed"; return -1;
  }
  const uint32_t durations[] = {0, 7, 0, 60};
  const uint16_t maximum = bits == 16 ? 65535 : 255;
  const uint16_t half = bits == 16 ? 32768 : 128;
  const uint16_t pixels[][8] = {
    {static_cast<uint16_t>(scenario == 2 ? maximum / 2 : maximum), 0, 0, maximum, 0, 0, maximum, maximum},
    {0, maximum, 0, half},
    {0, maximum, 0, maximum},
    {maximum, maximum, 0, half},
  };
  for (uint32_t index = 0; index < 4; ++index) {
    auto* settings = JxlEncoderFrameSettingsCreate(encoder.get(), nullptr);
    JxlFrameHeader frame;
    JxlEncoderInitFrameHeader(&frame);
    frame.duration = durations[index];
    frame.layer_info.have_crop = index ? JXL_TRUE : JXL_FALSE;
    frame.layer_info.xsize = index ? 1 : 2;
    frame.layer_info.ysize = 1;
    frame.layer_info.crop_x0 = index == 2 ? 1 : 0;
    const auto blend = scenario == 3 ? JXL_BLEND_ADD : (scenario == 4 ? JXL_BLEND_MUL :
        (scenario == 5 ? JXL_BLEND_MULADD : JXL_BLEND_BLEND));
    frame.layer_info.blend_info.blendmode = index == 1 || index == 3 ? blend : JXL_BLEND_REPLACE;
    frame.layer_info.blend_info.source = index == 0 ? 0 : (index == 1 || index == 2 ? 1 : 2);
    frame.layer_info.blend_info.alpha = 0;
    frame.layer_info.save_as_reference = index == 0 ? 1 : (index == 1 ? 2 : 0);
    const size_t samples = index ? 4 : 8;
    uint8_t eight[8];
    for (size_t sample = 0; sample < samples; ++sample) eight[sample] = static_cast<uint8_t>(pixels[index][sample]);
    JxlPixelFormat format{4, bits == 16 ? JXL_TYPE_UINT16 : JXL_TYPE_UINT8, JXL_NATIVE_ENDIAN, 0};
    if (!settings || JxlEncoderSetFrameLossless(settings, JXL_TRUE) != JXL_ENC_SUCCESS ||
        JxlEncoderSetExtraChannelDistance(settings, 0, 0) != JXL_ENC_SUCCESS ||
        JxlEncoderFrameSettingsSetOption(settings, JXL_ENC_FRAME_SETTING_EFFORT, 3) != JXL_ENC_SUCCESS ||
        JxlEncoderSetFrameHeader(settings, &frame) != JXL_ENC_SUCCESS ||
        JxlEncoderAddImageFrame(settings, &format, bits == 16 ? static_cast<const void*>(pixels[index]) : eight,
            samples * (bits / 8)) != JXL_ENC_SUCCESS) {
      error = "Fixture frame configuration failed"; return -1;
    }
  }
  JxlEncoderCloseInput(encoder.get());
  uint8_t* next = output;
  size_t remaining = sizeof(output);
  const auto status = JxlEncoderProcessOutput(encoder.get(), &next, &remaining);
  if (status != JXL_ENC_SUCCESS) {
    std::snprintf(detail, sizeof(detail), "Fixture output status %d, encoder error %d, remaining %zu",
        static_cast<int>(status), static_cast<int>(JxlEncoderGetError(encoder.get())), remaining);
    error = detail; return -1;
  }
  return static_cast<int>(sizeof(output) - remaining);
}
}
