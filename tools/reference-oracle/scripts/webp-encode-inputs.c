/* Fixture-only bridge to the pinned libwebp simple API (no codec implementation).
 * One request per process, little-endian u32 fields:
 * WPI1, width, height, codec (0=lossless/1=lossy), quality (0/75), RGBA length,
 * then RGBA bytes and EOF. Reply: WPO1, encoder version, decoder version,
 * encoded length, decoded length, encoded bytes, straight RGBA bytes.
 * The 16 MiB bound is this fixture protocol's limit, not a WebP claim.
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Public libwebp 1.3.2 encode.h/decode.h/types.h simple-API declarations.
 * No struct ABI, development package, or replacement codec library is needed.
 */
extern int WebPGetEncoderVersion(void);
extern int WebPGetDecoderVersion(void);
extern size_t WebPEncodeRGBA(const uint8_t *, int, int, int, float, uint8_t **);
extern size_t WebPEncodeLosslessRGBA(const uint8_t *, int, int, int, uint8_t **);
extern uint8_t *WebPDecodeRGBA(const uint8_t *, size_t, int *, int *);
extern void WebPFree(void *);

static int fail(const char *message) {
    fprintf(stderr, "webp-encode-inputs: %s\n", message);
    return 1;
}

static uint32_t word(const uint8_t *p) {
    return (uint32_t)p[0] | (uint32_t)p[1] << 8 |
           (uint32_t)p[2] << 16 | (uint32_t)p[3] << 24;
}

static void put(uint8_t *p, uint32_t value) {
    for (unsigned i = 0; i < 4; ++i) p[i] = (uint8_t)(value >> (8 * i));
}

int main(int argc, char **argv) {
    (void)argv;
    if (argc != 1) return fail("no command-line options supported");
    if (WebPGetEncoderVersion() != 0x010302 || WebPGetDecoderVersion() != 0x010302)
        return fail("requires libwebp encoder and decoder 1.3.2");
    uint8_t header[24];
    if (fread(header, 1, sizeof(header), stdin) != sizeof(header))
        return fail("truncated request header");
    if (memcmp(header, "WPI1", 4)) return fail("invalid protocol magic");
    uint32_t width = word(header + 4), height = word(header + 8);
    uint32_t codec = word(header + 12), quality = word(header + 16);
    uint32_t length = word(header + 20);
    if (!width || !height || width > 16383 || height > 16383)
        return fail("invalid dimensions");
    if (codec > 1 || quality != (codec ? 75u : 0u))
        return fail("unsupported codec/quality options");
    if (length > 16u * 1024u * 1024u || (uint64_t)width * height * 4 != length)
        return fail("invalid RGBA length");
    uint8_t *rgba = malloc(length), *encoded = NULL, *decoded = NULL;
    int result = 1;
    if (!rgba) return fail("allocation failed");
    if (fread(rgba, 1, length, stdin) != length) {
        fail("truncated RGBA"); goto done;
    }
    if (fgetc(stdin) != EOF || ferror(stdin)) {
        fail("trailing request data or input error"); goto done;
    }
    size_t size = codec
        ? WebPEncodeRGBA(rgba, (int)width, (int)height, (int)width * 4, 75.0f, &encoded)
        : WebPEncodeLosslessRGBA(rgba, (int)width, (int)height, (int)width * 4, &encoded);
    if (!size || !encoded || size > UINT32_MAX) {
        fail("encoding failed"); goto done;
    }
    int decoded_width = 0, decoded_height = 0;
    decoded = WebPDecodeRGBA(encoded, size, &decoded_width, &decoded_height);
    if (!decoded || decoded_width != (int)width || decoded_height != (int)height) {
        fail("decode failed or dimensions changed"); goto done;
    }
    uint8_t reply[20] = {'W', 'P', 'O', '1'};
    put(reply + 4, (uint32_t)WebPGetEncoderVersion());
    put(reply + 8, (uint32_t)WebPGetDecoderVersion());
    put(reply + 12, (uint32_t)size);
    put(reply + 16, length);
    if (fwrite(reply, 1, sizeof(reply), stdout) != sizeof(reply) ||
        fwrite(encoded, 1, size, stdout) != size ||
        fwrite(decoded, 1, length, stdout) != length || fflush(stdout)) {
        fail("output failed"); goto done;
    }
    result = 0;
done:
    WebPFree(decoded);
    WebPFree(encoded);
    free(rgba);
    return result;
}
