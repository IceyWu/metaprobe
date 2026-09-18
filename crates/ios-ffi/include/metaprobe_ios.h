#ifndef METAPROBE_IOS_H
#define METAPROBE_IOS_H

#include <stddef.h>

char *metaprobe_extract_json(const unsigned char *data, size_t data_len, const char *filename);
void metaprobe_free_string(char *value);

#endif
