#include "ring_buffer.h"

void ring_buffer_init(ring_buffer_t *rb, int16_t *buf, size_t capacity) {
    rb->buffer = buf;
    rb->capacity = capacity;
    rb->head = 0;
    rb->tail = 0;
}

size_t ring_buffer_available(const ring_buffer_t *rb) {
    size_t head = rb->head;
    size_t tail = rb->tail;
    if (head >= tail) {
        return head - tail;
    }
    return rb->capacity - (tail - head);
}

size_t ring_buffer_space(const ring_buffer_t *rb) {
    return rb->capacity - ring_buffer_available(rb);
}

size_t ring_buffer_write(ring_buffer_t *rb, const int16_t *data, size_t len) {
    size_t space = ring_buffer_space(rb);
    if (len > space) {
        len = space;
    }
    for (size_t i = 0; i < len; ++i) {
        rb->buffer[rb->head] = data[i];
        rb->head = (rb->head + 1) % rb->capacity;
    }
    return len;
}

size_t ring_buffer_read(ring_buffer_t *rb, int16_t *data, size_t len) {
    size_t avail = ring_buffer_available(rb);
    if (len > avail) {
        len = avail;
    }
    for (size_t i = 0; i < len; ++i) {
        data[i] = rb->buffer[rb->tail];
        rb->tail = (rb->tail + 1) % rb->capacity;
    }
    return len;
}
