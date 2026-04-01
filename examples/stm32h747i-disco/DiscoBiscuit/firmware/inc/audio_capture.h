#ifndef AUDIO_CAPTURE_H
#define AUDIO_CAPTURE_H

#include "ring_buffer.h"
#include "stm32h7xx_hal.h"
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define AUDIO_CAPTURE_FRAME_SAMPLES 512

void audio_capture_init(ring_buffer_t *rb);

#ifdef __cplusplus
}
#endif

#endif /* AUDIO_CAPTURE_H */
