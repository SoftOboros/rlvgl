#ifndef RING_BUFFER_H
#define RING_BUFFER_H

#include <stddef.h>
#include <stdint.h>

typedef struct {
    int16_t *buffer;
    size_t capacity;
    volatile size_t head;
    volatile size_t tail;
} ring_buffer_t;

void ring_buffer_init(ring_buffer_t *rb, int16_t *buf, size_t capacity);
size_t ring_buffer_write(ring_buffer_t *rb, const int16_t *data, size_t len);
size_t ring_buffer_read(ring_buffer_t *rb, int16_t *data, size_t len);
size_t ring_buffer_available(const ring_buffer_t *rb);
size_t ring_buffer_space(const ring_buffer_t *rb);

#endif /* RING_BUFFER_H */
