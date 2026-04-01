#include "audio_capture.h"
#include "command_router.h"
#include "kws_inference.h"
#include "ring_buffer.h"
#include <stdio.h>

#ifndef SMOOTHING_ALPHA
#define SMOOTHING_ALPHA 0.2f
#endif

static int16_t rb_storage[AUDIO_CAPTURE_FRAME_SAMPLES * 4];
static ring_buffer_t audio_rb;
static float smooth_scores[KWS_NUM_CLASSES];
static int smoothing_init;

int main(void) {
    ring_buffer_init(&audio_rb, rb_storage, AUDIO_CAPTURE_FRAME_SAMPLES * 4);
    audio_capture_init(&audio_rb);
    kws_inference_init();

    /* Demo command: type=1, len=1, payload=0x42 */
    const uint8_t demo_cmd[] = {1, 1, 0x42};
    for (size_t i = 0; i < sizeof(demo_cmd); ++i) {
        command_router_feed(demo_cmd[i]);
    }

    int16_t frame[AUDIO_CAPTURE_FRAME_SAMPLES];
    float scores[KWS_NUM_CLASSES];

    while (1) {
        if (ring_buffer_available(&audio_rb) >= AUDIO_CAPTURE_FRAME_SAMPLES) {
            ring_buffer_read(&audio_rb, frame, AUDIO_CAPTURE_FRAME_SAMPLES);
            kws_inference_run(frame, AUDIO_CAPTURE_FRAME_SAMPLES, scores, KWS_NUM_CLASSES);
            if (!smoothing_init) {
                for (size_t i = 0; i < KWS_NUM_CLASSES; ++i) {
                    smooth_scores[i] = scores[i];
                }
                smoothing_init = 1;
            } else {
                for (size_t i = 0; i < KWS_NUM_CLASSES; ++i) {
                    smooth_scores[i] =
                        SMOOTHING_ALPHA * scores[i] + (1.0f - SMOOTHING_ALPHA) * smooth_scores[i];
                }
            }
            int top = 0;
            for (size_t i = 1; i < KWS_NUM_CLASSES; ++i) {
                if (smooth_scores[i] > smooth_scores[top]) {
                    top = (int)i;
                }
            }
            printf("[KWS] Top: %s (%.2f)\n", kws_class_names[top], smooth_scores[top]);
            for (size_t i = 0; i < KWS_NUM_CLASSES; ++i) {
                printf("  %s: %.2f\n", kws_class_names[i], smooth_scores[i]);
            }
        }
        command_router_poll();
    }

    return 0;
}

void Error_Handler(void) {
    while (1) {
    }
}
