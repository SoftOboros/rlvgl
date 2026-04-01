#ifndef COMMAND_ROUTER_H
#define COMMAND_ROUTER_H

#include <stddef.h>
#include <stdint.h>

#define COMMAND_ROUTER_BUFFER_SIZE 64

void command_router_feed(uint8_t byte);
void command_router_poll(void);

__attribute__((weak)) void command_router_handle(uint8_t type, const uint8_t *payload,
                                                 uint8_t length);

#endif // COMMAND_ROUTER_H
