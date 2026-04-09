#include "kws_inference.h"

#include <string.h>

const char *const kws_class_names[KWS_NUM_CLASSES] = {
    "open_channel",
    "close_channel",
    "noise",
    "unknown",
};

int kws_inference_init(void) { return 0; }

int kws_inference_run(const int16_t *pcm, size_t sample_count, float *scores, size_t score_count) {
    (void)pcm;
    (void)sample_count;

    static const float dummy_scores[KWS_NUM_CLASSES] = {0.7f, 0.2f, 0.1f, 0.0f};
    size_t count = score_count < KWS_NUM_CLASSES ? score_count : KWS_NUM_CLASSES;
    if (scores) {
        for (size_t i = 0; i < count; ++i) {
            scores[i] = dummy_scores[i];
        }
    }

    int top_idx = 0;
    for (size_t i = 1; i < count; ++i) {
        if (scores && scores[i] > scores[top_idx]) {
            top_idx = (int)i;
        }
    }
    return top_idx;
}
