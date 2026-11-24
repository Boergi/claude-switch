"""Create the geometric app icon without external image dependencies."""
import math
from pathlib import Path
import struct
import zlib

size = 1024
segments = [((280, 365), (748, 365)), ((748, 365), (620, 237)),
            ((744, 659), (276, 659)), ((276, 659), (404, 787))]


def distance(x, y, start, end):
    dx, dy = end[0] - start[0], end[1] - start[1]
    t = max(0, min(1, ((x - start[0]) * dx + (y - start[1]) * dy) / (dx * dx + dy * dy)))
    return math.hypot(x - start[0] - t * dx, y - start[1] - t * dy)


rows = bytearray()
for y in range(size):
    rows.append(0)
    for x in range(size):
        radius = math.hypot(max(abs(x - 511.5) - 288, 0), max(abs(y - 511.5) - 288, 0))
        alpha = round(255 * max(0, min(1, 176 - radius)))
        stroke = max(0, min(1, 32 - min(distance(x, y, a, b) for a, b in segments)))
        background, foreground = (56, 79, 65), (247, 246, 237)
        rows.extend(round(a + stroke * (b - a)) for a, b in zip(background, foreground))
        rows.append(alpha)


def chunk(kind, data):
    return struct.pack('!I', len(data)) + kind + data + struct.pack('!I', zlib.crc32(kind + data))


png = b'\x89PNG\r\n\x1a\n'
png += chunk(b'IHDR', struct.pack('!2I5B', size, size, 8, 6, 0, 0, 0))
png += chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b'')
(Path(__file__).parent.parent / 'src-tauri/icons/icon.png').write_bytes(png)
