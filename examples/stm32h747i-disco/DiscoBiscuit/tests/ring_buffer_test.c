#include "ring_buffer.h"
#include <assert.h>
#include <stdio.h>

int main(void) {
    int16_t storage[4];
    ring_buffer_t rb;
    ring_buffer_init(&rb, storage, 4);

    int16_t input[3] = {1, 2, 3};
    assert(ring_buffer_write(&rb, input, 3) == 3);
    assert(ring_buffer_available(&rb) == 3);

    int16_t output[3] = {0};
    assert(ring_buffer_read(&rb, output, 3) == 3);
    assert(output[0] == 1 && output[1] == 2 && output[2] == 3);
    assert(ring_buffer_available(&rb) == 0);

    puts("ring_buffer_test_passed");
    return 0;
}
