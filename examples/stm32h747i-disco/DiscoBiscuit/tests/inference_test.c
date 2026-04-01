#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <assert.h>

#include "kws_inference.h"

static int16_t *load_wav(const char *path, size_t *out_samples) {
    FILE *f = fopen(path, "rb");
    if (!f) {
        perror("fopen");
        return NULL;
    }
    unsigned char header[44];
    if (fread(header, 1, sizeof(header), f) != sizeof(header)) {
        fclose(f);
        return NULL;
    }
    if (memcmp(header, "RIFF", 4) || memcmp(header + 8, "WAVE", 4)) {
        fclose(f);
        return NULL;
    }
    uint32_t data_chunk_size = 0;
    // find "data" chunk
    long pos = 12;
    while (1) {
        fseek(f, pos, SEEK_SET);
        if (fread(header, 1, 8, f) != 8) {
            fclose(f);
            return NULL;
        }
        uint32_t chunk_size = header[4] | (header[5] << 8) | (header[6] << 16) | (header[7] << 24);
        if (!memcmp(header, "data", 4)) {
            data_chunk_size = chunk_size;
            break;
        }
        pos += 8 + chunk_size;
    }
    int16_t *pcm = malloc(data_chunk_size);
    if (!pcm) {
        fclose(f);
        return NULL;
    }
    if (fread(pcm, 1, data_chunk_size, f) != data_chunk_size) {
        free(pcm);
        fclose(f);
        return NULL;
    }
    fclose(f);
    *out_samples = data_chunk_size / 2;
    return pcm;
}

int main(void) {
    if (kws_inference_init() != 0) {
        fprintf(stderr, "init failed\n");
        return 1;
    }
    size_t sample_count = 0;
    // Generated on demand by tests/gen_open_channel_wav.py to avoid storing binary assets.
    int16_t *pcm = load_wav("tests/fixtures/open_channel.wav", &sample_count);
    if (!pcm) {
        fprintf(stderr, "failed to load wav\n");
        return 1;
    }
    float scores[KWS_NUM_CLASSES] = {0};
    int top = kws_inference_run(pcm, sample_count, scores, KWS_NUM_CLASSES);
    free(pcm);
    if (top != 0) {
        fprintf(stderr, "expected class 0 but got %d\n", top);
        return 1;
    }
    printf("inference_test passed: %s\n", kws_class_names[top]);
    return 0;
}
