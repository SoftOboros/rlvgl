#include "audio_capture.h"

static int16_t s_dma_buffer[2][AUDIO_CAPTURE_FRAME_SAMPLES];
static ring_buffer_t *s_ring;
static DFSDM_Filter_HandleTypeDef hdfsdm1_filter0;

void audio_capture_init(ring_buffer_t *rb) {
    s_ring = rb;
    /* Start DFSDM filter DMA in ping-pong mode */
    HAL_DFSDM_FilterRegularStart_DMA(&hdfsdm1_filter0, (int32_t *)s_dma_buffer,
                                     AUDIO_CAPTURE_FRAME_SAMPLES * 2);
}

void HAL_DFSDM_FilterRegConvHalfCpltCallback(DFSDM_Filter_HandleTypeDef *hdfsdm) {
    (void)hdfsdm;
    if (s_ring) {
        ring_buffer_write(s_ring, s_dma_buffer[0], AUDIO_CAPTURE_FRAME_SAMPLES);
    }
}

void HAL_DFSDM_FilterRegConvCpltCallback(DFSDM_Filter_HandleTypeDef *hdfsdm) {
    (void)hdfsdm;
    if (s_ring) {
        ring_buffer_write(s_ring, s_dma_buffer[1], AUDIO_CAPTURE_FRAME_SAMPLES);
    }
}
