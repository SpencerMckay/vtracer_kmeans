#ifndef PNG_TO_SVG_RUST_H
#define PNG_TO_SVG_RUST_H

#include <stdint.h>

char* convert_png_to_svg(
    const uint8_t* image_data,
    uintptr_t image_size,
    const char* params_json);

void free_cstring(char* s);

#endif /* PNG_TO_SVG_RUST_H */