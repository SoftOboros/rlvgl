#ifndef KWS_INFERENCE_H
#define KWS_INFERENCE_H

#include <stddef.h>
#include <stdint.h>

#define KWS_NUM_CLASSES 4

extern const char *const kws_class_names[KWS_NUM_CLASSES];

#ifdef __cplusplus
extern "C" {
#endif

int kws_inference_init(void);
int kws_inference_run(const int16_t *pcm, size_t sample_count, float *scores, size_t score_count);

#ifdef __cplusplus
}
#endif

#endif /* KWS_INFERENCE_H */
