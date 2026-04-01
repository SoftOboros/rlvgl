#include "command_router.h"

#include <stdio.h>
#include <string.h>

static uint8_t s_buffer[COMMAND_ROUTER_BUFFER_SIZE];
static size_t s_len;

void command_router_feed(uint8_t byte) {
    if (s_len < COMMAND_ROUTER_BUFFER_SIZE) {
        s_buffer[s_len++] = byte;
    } else {
        s_len = 0; // overflow, drop data
    }
}

void command_router_poll(void) {
    while (s_len >= 2) {
        uint8_t type = s_buffer[0];
        uint8_t len = s_buffer[1];
        if (s_len < (size_t)(2 + len)) {
            break;
        }
        command_router_handle(type, &s_buffer[2], len);
        memmove(s_buffer, &s_buffer[2 + len], s_len - (2 + len));
        s_len -= 2 + len;
    }
}

__attribute__((weak)) void command_router_handle(uint8_t type, const uint8_t *payload,
                                                 uint8_t length) {
    printf("[CMD] type=%u len=%u\n", type, (unsigned)length);
    for (uint8_t i = 0; i < length; ++i) {
        printf(" %02x", payload[i]);
    }
    if (length) {
        printf("\n");
    }
}
