#!/usr/bin/env python3
# Strict format checks for the files in a community package (community.py runs them on the
# server before listing). Each check reads only what it needs - headers, directories, lump and
# table entries - through seek+read on a file, never the whole file, and never decompresses
# anything. A file that doesn't check out raises FormatError with a plain reason.
#
# What's checked, per kind (the game folder path decides which):
#   .bsp  version 30; every lump inside the file and a whole number of records; counts within
#         the engine's own index types (16-bit node/leaf/clipnode/face/vertex indices...); every
#         index between lumps in range; embedded textures inside the textures lump, 16-aligned
#   .wad  WAD3; directory and every entry inside the file; known entry types, no compression;
#         texture entries' sizes and mip offsets inside the entry
#   .mdl  IDST version 10 (IDSQ for sequence group files); every table inside the file; bodypart
#         models, meshes, vertices and textures inside the file; sequence groups and events
#   .spr  IDSP version 2; palette; every frame (and frame group) inside the file
#   .wav  RIFF/WAVE, PCM 8/16-bit mono/stereo, fmt and data chunks inside the file
#   .mp3, .png, .jpg  the right signature; .bmp / .tga  header, size and pixel data inside
#   .nav  the nav magic and a version the game reads; .lsv  the bot brain's magic and version
#   .res .txt .ini .json .map  text without NUL bytes (JSON must parse)
# Limits are the engine's (index types, MAXSTUDIO*), kept wide enough for real maps: checked on
# every map, model, sprite, sound and texture of the installed CS 1.6, CZ and Half-Life, the
# downloads folder, our nav pack and real GameBanana packages without one false refusal
# (docs/COMMUNITY-SERVER.md, "Format checks").
import json
import struct
import time

MAX_TRAILING = 1 << 20      # bytes after a file's last structure (padding some tools leave)
MAX_WAV_CHUNKS = 4096
MAX_PIXELS = 4096 * 4096    # the biggest picture looked at (RLE packets are walked one by one)
MAX_MESHES = 1 << 16        # meshes in a whole model

_deadline = None


class FormatError(Exception):
    pass


def set_deadline(t):
    """A time.monotonic() after which every check stops with "took too long" (None: no limit).
    The runner's CPU limit and the service's kill are the real limits; this is a third."""
    global _deadline
    _deadline = t


def tick():
    if _deadline is not None and time.monotonic() > _deadline:
        raise FormatError("it took too long to check")


class Reader:
    """Reads exact byte ranges of a file (seek + read), refusing anything outside it."""

    def __init__(self, f, size):
        self.f, self.size = f, size

    def at(self, off, n):
        tick()
        if off < 0 or n < 0 or off + n > self.size:
            raise FormatError("a part of it points past the end of the file")
        self.f.seek(off)
        b = self.f.read(n)
        if len(b) != n:
            raise FormatError("it ends early")
        return b

    def ints(self, off, fmt):
        return struct.unpack("<" + fmt, self.at(off, struct.calcsize("<" + fmt)))

    def inside(self, off, n, what):
        if off < 0 or n < 0 or off + n > self.size:
            raise FormatError(f"its {what} goes past the end of the file")


def _need(cond, why):
    if not cond:
        raise FormatError(why)


# ---- BSP (version 30) -------------------------------------------------------------------------

BSP_LUMPS = ("entities", "planes", "textures", "vertices", "visibility", "nodes", "texinfo", "faces",
             "lighting", "clipnodes", "leaves", "mark surfaces", "edges", "surface edges", "models")
# record size and the most the engine can index, per lump
BSP_RECORDS = {1: (20, 65536), 3: (12, 65536), 5: (24, 32768), 6: (40, 32768), 7: (20, 65536), 9: (8, 32768),
               10: (28, 32768), 11: (2, 65536), 12: (4, 1 << 21), 13: (4, 1 << 22), 14: (64, 512)}
CHUNK = 1 << 16


def _records(r, lump, fmt):
    """Yields the records of a lump, reading it in pieces."""
    off, length = lump
    size = struct.calcsize("<" + fmt)
    per = max(1, CHUNK // size)
    for start in range(0, length // size, per):
        tick()
        n = min(per, length // size - start)
        data = r.at(off + start * size, n * size)
        yield from struct.iter_unpack("<" + fmt, data)


def check_bsp(r):
    _need(r.size >= 124, "it's too small to be a map")
    version = r.ints(0, "i")[0]
    # 30 is Half-Life's; the engine also reads Quake's 29 (stock Half-Life ships one)
    _need(version in (29, 30), f"it's a version {version} map; the game reads version 30")
    raw = r.ints(4, "30i")
    lumps = [(raw[2 * i], raw[2 * i + 1]) for i in range(15)]
    end = 124
    for i, (off, length) in enumerate(lumps):
        _need(length >= 0 and (length == 0 or off >= 124), f"its {BSP_LUMPS[i]} are damaged")
        r.inside(off, length, BSP_LUMPS[i])
        if i in BSP_RECORDS:
            size, most = BSP_RECORDS[i]
            _need(length % size == 0, f"its {BSP_LUMPS[i]} are damaged")
            _need(length // size <= most, f"it has more {BSP_LUMPS[i]} than the game allows ({length // size}, at most {most})")
        end = max(end, off + length)
    _need(r.size - end <= MAX_TRAILING, "it has unexplained data after the map")
    n = {i: lumps[i][1] // BSP_RECORDS[i][0] for i in BSP_RECORDS}
    vis_len, light_len = lumps[4][1], lumps[8][1]
    _need(n[14] >= 1, "it has no world model")

    # entities: text, NUL only at the end, balanced braces
    off, length = lumps[0]
    _need(length <= 4 << 20, "its entity list is too big")
    ents = r.at(off, length).rstrip(b"\0")
    _need(b"\0" not in ents, "its entity list is damaged")
    depth, quoted = 0, False
    for c in ents:
        if c == 0x22:
            quoted = not quoted
        elif c == 0x0A:
            quoted = False  # a value never runs past the end of its line
        elif quoted:
            continue
        elif c == 0x7B:
            depth += 1
            _need(depth == 1, "its entity list is damaged")
        elif c == 0x7D:
            depth -= 1
            _need(depth == 0, "its entity list is damaged")
    _need(depth == 0, "its entity list is damaged")

    ntex = check_bsp_textures(r, lumps[2])
    for (planenum, side, firstedge, numedges, texinfo, s0, s1, s2, s3, lightofs) in _records(r, lumps[7], "HHiHh4Bi"):
        _need(planenum < n[1] and 0 <= texinfo < n[6], "a face points at something that isn't there")
        _need(firstedge >= 0 and firstedge + numedges <= n[13], "a face points at edges that aren't there")
        _need(lightofs == -1 or 0 <= lightofs <= light_len, "a face's lighting is outside the lighting data")
    for (e,) in _records(r, lumps[13], "i"):
        _need(-n[12] < e < n[12], "a surface edge points at an edge that isn't there")
    for (v0, v1) in _records(r, lumps[12], "HH"):
        _need(v0 < n[3] and v1 < n[3], "an edge points at a vertex that isn't there")
    for rec in _records(r, lumps[6], "8fii"):
        _need(ntex == 0 or 0 <= rec[8] < ntex, "a texture alignment points at a texture that isn't there")
    for (plane, c0, c1, *box, firstface, numfaces) in _records(r, lumps[5], "ihh6hHH"):
        _need(0 <= plane < n[1], "a node points at a plane that isn't there")
        for c in (c0, c1):
            _need((c >= 0 and c < n[5]) or (c < 0 and -(c + 1) < n[10]), "a node points at something that isn't there")
        _need(firstface + numfaces <= n[7], "a node points at faces that aren't there")
    for (contents, visofs, *box, firstmark, nummark, a0, a1, a2, a3) in _records(r, lumps[10], "ii6hHH4B"):
        _need(visofs == -1 or 0 <= visofs <= vis_len, "a leaf's visibility is outside the visibility data")
        _need(firstmark + nummark <= n[11], "a leaf points at surfaces that aren't there")
    for (m,) in _records(r, lumps[11], "H"):
        _need(m < n[7], "a mark surface points at a face that isn't there")
    for (plane, c0, c1) in _records(r, lumps[9], "ihh"):
        _need(0 <= plane < n[1], "a clip node points at a plane that isn't there")
        for c in (c0, c1):
            _need((0 <= c < n[9]) or -16 <= c < 0, "a clip node points at something that isn't there")
    for rec in _records(r, lumps[14], "9f4iiii"):
        heads, firstface, numfaces = rec[9:13], rec[14], rec[15]
        _need(0 <= heads[0] < max(n[5], 1), "a model points at a node that isn't there")
        # (compilers set an unused hull's head to the clip node count)
        _need(all(h <= n[9] for h in heads[1:]), "a model points at a clip node that isn't there")
        _need(firstface >= 0 and numfaces >= 0 and firstface + numfaces <= n[7], "a model points at faces that aren't there")
    return "map"


def _miptex(r, base, limit, what):
    """Checks one miptex record at base, with its data inside [base, base+limit)."""
    _need(limit >= 40, f"{what} is damaged")
    name = r.at(base, 16)
    w, h, *offs = r.ints(base + 16, "6I")
    _need(0 < w <= 4096 and 0 < h <= 4096, f"{what} has an impossible size ({w}x{h})")
    _need(w % 16 == 0 and h % 16 == 0, f"{what} isn't a multiple of 16 in size ({w}x{h})")
    if offs[0]:
        # the engine reads the four mip levels as one block from the first offset (the other
        # offsets are often junk in real files and aren't used), then the palette after it
        _need(40 <= offs[0] and offs[0] + w * h * 85 // 64 <= limit, f"{what}'s pixels are outside it")
    return name


def check_bsp_textures(r, lump):
    off, length = lump
    if length == 0:
        return 0
    _need(length >= 4, "its textures are damaged")
    count = r.ints(off, "i")[0]
    _need(0 <= count <= 4096 and 4 + 4 * count <= length, "its textures are damaged")
    offs = r.ints(off + 4, f"{count}i") if count else ()
    for i, o in enumerate(offs):
        if o == -1:
            continue
        _need(0 <= o and o + 40 <= length, "a texture is outside the textures")
        _miptex(r, off + o, length - o, f"texture {i}")
    return count


# ---- WAD3 ----------------------------------------------------------------------------------------

WAD_TYPES = {0x40: "decal", 0x42: "picture", 0x43: "texture", 0x46: "font"}


def check_wad(r):
    _need(r.size >= 12, "it's too small to be a texture file")
    magic = r.at(0, 4)
    _need(magic == b"WAD3", "it isn't a WAD3 texture file")
    count, diroff = r.ints(4, "ii")
    _need(0 <= count <= 1 << 16, "its directory is damaged")
    r.inside(diroff, count * 32, "directory")
    end = diroff + count * 32
    for i in range(count):
        filepos, disksize, size, typ, comp, _p, name = struct.unpack("<iiiBBH16s", r.at(diroff + i * 32, 32))
        label = name.split(b"\0")[0].decode("latin-1") or f"entry {i}"
        _need(comp == 0, f"{label} is compressed, which the game doesn't read")
        _need(typ in WAD_TYPES, f"{label} is of an unknown kind")
        _need(filepos >= 12 and disksize >= 0, f"{label} is damaged")
        r.inside(filepos, disksize, label)
        if typ in (0x40, 0x43):
            _miptex(r, filepos, disksize, label)
        end = max(end, filepos + disksize)
    _need(r.size - end <= MAX_TRAILING, "it has unexplained data after the textures")
    return "wad"


# ---- MDL (IDST v10, IDSQ v10) ------------------------------------------------------------------------

MDL_TABLES = (  # (count field, index field, record size, most, what)
    (0, 1, 112, 256, "bones"), (2, 3, 24, 64, "bone controllers"), (4, 5, 32, 1024, "hit boxes"),
    (6, 7, 176, 4096, "sequences"), (8, 9, 104, 64, "sequence groups"), (10, 11, 80, 512, "textures"),
    (16, 17, 76, 64, "body parts"), (18, 19, 88, 512, "attachments"))


def check_mdl(r):
    _need(r.size >= 8, "it's too small to be a model")
    magic, version = r.at(0, 4), r.ints(4, "i")[0]
    _need(magic in (b"IDST", b"IDSQ"), "it isn't a GoldSrc model")
    _need(version == 10, f"it's a version {version} model; the game reads version 10")
    _need(r.size >= 76, "it's too small to be a model")
    # the length the model says it has (real ones are off by a few bytes at most: a palette
    # cut 4 bytes short, 2 bytes of padding)
    length = r.ints(72, "i")[0]
    _need(length <= r.size + 768, "it's shorter than it says: it was cut off")
    _need(r.size - length <= MAX_TRAILING, "it has unexplained data after the model")
    if magic == b"IDSQ":
        return "model"
    _need(r.size >= 244, "it's too small to be a model")
    h = r.ints(140, "26i")
    for c, i, size, most, what in MDL_TABLES:
        _need(0 <= h[c] <= most, f"it has more {what} than the game allows ({h[c]})")
        if h[c]:
            r.inside(h[i], h[c] * size, what)
    numbones, numseq, numgroups, numtex = h[0], h[6], h[8], h[10]
    numskinref, numfamilies, skinindex = h[13], h[14], h[15]
    _need(0 <= numskinref <= 512 and 0 <= numfamilies <= 128, "its skins are damaged")
    if numskinref and numfamilies:
        r.inside(skinindex, numskinref * numfamilies * 2, "skins")
    _need(0 <= h[24] <= 512, "its transitions are damaged")
    if h[24]:
        r.inside(h[25], h[24] * h[24], "transitions")
    # textures and their pixels
    for t in range(numtex):
        flags, w, ht, index = r.ints(h[11] + t * 80 + 64, "4i")
        _need(0 < w <= 4096 and 0 < ht <= 4096, f"texture {t} has an impossible size")
        if index:
            # its pixels (the 768-byte palette after them is sometimes cut a little short in
            # real models, which the game copes with)
            r.inside(index, w * ht, f"texture {t}")
    # body parts -> models -> meshes and vertices
    meshes = 0
    for b in range(h[16]):
        nummodels, base, modelindex = r.ints(h[17] + b * 76 + 64, "3i")
        _need(0 <= nummodels <= 64, "a body part is damaged")
        if nummodels:
            r.inside(modelindex, nummodels * 112, "body part")
        for m in range(nummodels):
            (typ, radius, nummesh, meshindex, numverts, vertinfo, vertindex, numnorms, norminfo, normindex,
             numgroups_, groupindex) = r.ints(modelindex + m * 112 + 64, "ifiiiiiiiiii")
            _need(0 <= nummesh <= 1024 and 0 <= numverts <= 1 << 16 and 0 <= numnorms <= 1 << 16, "a body model is damaged")
            for off, n in ((meshindex, nummesh * 20), (vertindex, numverts * 12), (vertinfo, numverts),
                           (normindex, numnorms * 12), (norminfo, numnorms)):
                if n:
                    r.inside(off, n, "body model")
            meshes += nummesh
            _need(meshes <= MAX_MESHES, "it has more meshes than the game allows")
            for k in range(nummesh):
                numtris, triindex, skinref, mnorms, mnormindex = r.ints(meshindex + k * 20, "5i")
                _need(numtris >= 0 and (numtris == 0 or 0 <= triindex < r.size), "a mesh is damaged")
                # (a model whose textures are in a separate ...T.mdl has no skin table of its own)
                _need(numskinref == 0 or 0 <= skinref < numskinref, "a mesh points at a skin that isn't there")
    # sequences: their group and events, and in-file animations
    for s in range(numseq):
        base = h[7] + s * 176
        numevents, eventindex = r.ints(base + 48, "ii")
        numblends, animindex = r.ints(base + 120, "ii")
        seqgroup = r.ints(base + 156, "i")[0]
        _need(0 <= numevents <= 1024, "a sequence is damaged")
        if numevents:
            r.inside(eventindex, numevents * 76, "sequence events")
        _need(0 <= seqgroup < max(numgroups, 1), "a sequence points at a group that isn't there")
        _need(1 <= numblends <= 16, "a sequence is damaged")
        if seqgroup == 0 and numbones:
            r.inside(animindex, numblends * numbones * 12, "animation")
    return "model"


# ---- SPR (IDSP v2) -------------------------------------------------------------------------------------

def check_spr(r):
    _need(r.size >= 42, "it's too small to be a sprite")
    _need(r.at(0, 4) == b"IDSP", "it isn't a GoldSrc sprite")
    version, typ, fmt, radius, maxw, maxh, frames, beam, sync = r.ints(4, "iiifiiifi")
    _need(version == 2, f"it's a version {version} sprite; the game reads version 2")
    _need(0 <= typ <= 4 and 0 <= fmt <= 3, "its header is damaged")
    _need(0 < frames <= 1000, f"it has an impossible number of frames ({frames})")
    colors = r.ints(40, "H")[0]
    _need(0 < colors <= 256, "its palette is damaged")
    pos = 42 + colors * 3
    r.inside(42, colors * 3, "palette")

    def frame(pos):
        ox, oy, w, h = r.ints(pos, "4i")
        _need(0 < w <= 4096 and 0 < h <= 4096, f"a frame has an impossible size ({w}x{h})")
        r.inside(pos + 16, w * h, "frame")
        return pos + 16 + w * h

    for _ in range(frames):
        kind = r.ints(pos, "i")[0]
        pos += 4
        if kind == 0:
            pos = frame(pos)
        elif kind == 1:
            n = r.ints(pos, "i")[0]
            _need(0 < n <= 1000, "a frame group is damaged")
            r.inside(pos + 4, n * 4, "frame group")
            pos += 4 + n * 4
            for _ in range(n):
                pos = frame(pos)
        else:
            raise FormatError("a frame is of an unknown kind")
    _need(r.size - pos <= MAX_TRAILING, "it has unexplained data after the frames")
    return "sprite"


# ---- sounds --------------------------------------------------------------------------------------------

def check_wav(r):
    _need(r.size >= 12 and r.at(0, 4) == b"RIFF" and r.at(8, 4) == b"WAVE", "it isn't a WAV sound")
    pos, fmt, data, chunks = 12, None, None, 0
    while pos + 8 <= r.size:
        chunks += 1
        _need(chunks <= MAX_WAV_CHUNKS, "it has too many parts")
        cid, size = r.at(pos, 4), r.ints(pos + 4, "I")[0]
        if cid == b"fmt ":
            _need(size >= 16, "its format is damaged")
            fmt = r.ints(pos + 8, "HHIIHH")
        elif cid == b"data":
            r.inside(pos + 8, size, "sound data")
            data = (pos + 8, size)
            if fmt is not None:
                break  # (chunks after the sound are often cut short in real files, the stock ones too)
        _need(size <= r.size, "a part of it claims to be bigger than the file")
        pos += 8 + size + (size & 1)
    _need(fmt is not None, "it has no format")
    _need(data is not None, "it has no sound data")
    tag, channels, rate, _bps, _align, bits = fmt
    # PCM, or 32-bit float (two of the stock radio sounds are)
    _need((tag == 1 and bits in (8, 16, 24)) or (tag == 3 and bits == 32), "it isn't plain PCM sound, which is what the game plays")
    _need(channels in (1, 2) and 1000 <= rate <= 96000, "its format is one the game doesn't play")
    return "sound"


def check_mp3(r):
    # an ID3 tag or an MPEG frame, after any zero padding (the stock music has some)
    head = r.at(0, min(r.size, 64 * 1024)).lstrip(b"\0")
    _need(len(head) >= 3 and (head[:3] == b"ID3" or (head[0] == 0xFF and head[1] & 0xE0 == 0xE0)), "it isn't an MP3")
    return "sound"


# ---- pictures --------------------------------------------------------------------------------------------

def check_bmp(r):
    _need(r.size >= 26 and r.at(0, 2) == b"BM", "it isn't a BMP picture")
    pixels, dib = r.ints(10, "II")
    if dib == 12:
        w, h, planes, bpp = r.ints(18, "HHHH")
        comp = 0
    else:
        _need(dib >= 40, "its header is damaged")
        w, h, planes, bpp, comp = r.ints(18, "iiHHI")
    h = abs(h)
    _need(0 < w <= 8192 and 0 < h <= 8192, f"it has an impossible size ({w}x{h})")
    _need(bpp in (1, 4, 8, 16, 24, 32) and comp in (0, 1, 2, 3), "its format is damaged")
    _need(14 + dib <= pixels < r.size, "its pixels are outside it")
    if comp in (0, 3):
        row = ((w * bpp + 31) // 32) * 4
        _need(pixels + row * h <= r.size, "its pixels go past the end of the file")
    return "picture"


def check_tga(r):
    _need(r.size >= 18, "it's too small to be a TGA picture")
    idlen, cmtype, imtype = r.ints(0, "BBB")
    cmfirst, cmlen, cmbits = r.ints(3, "HHB")
    w, h, bpp = r.ints(12, "HHB")
    _need(cmtype in (0, 1) and imtype in (1, 2, 3, 9, 10, 11), "it isn't a TGA picture the game reads")
    _need(0 < w <= 8192 and 0 < h <= 8192, f"it has an impossible size ({w}x{h})")
    _need(bpp in (8, 15, 16, 24, 32), "its format is damaged")
    start = 18 + idlen + (cmlen * ((cmbits + 7) // 8) if cmtype else 0)
    _need(start <= r.size, "it ends early")
    px = (bpp + 7) // 8
    if imtype in (1, 2, 3):
        _need(start + w * h * px <= r.size, "its pixels go past the end of the file")
    else:
        # run-length packets: walk them until every pixel is accounted for, inside the file
        _need(r.size <= 64 << 20 and w * h <= MAX_PIXELS, "it's too big for a picture")
        data, pos, left, packets = r.at(0, r.size), start, w * h, 0
        while left > 0:
            packets += 1
            if packets & 0xFFFF == 0:
                tick()
            _need(pos < len(data), "its pixels go past the end of the file")
            head = data[pos]
            n = (head & 0x7F) + 1
            pos += 1 + (px if head & 0x80 else n * px)
            left -= n
        _need(pos <= len(data), "its pixels go past the end of the file")
    return "picture"


def check_png(r):
    _need(r.at(0, min(8, r.size)) == b"\x89PNG\r\n\x1a\n", "it isn't a PNG picture")
    return "picture"


def check_jpg(r):
    _need(r.at(0, min(3, r.size)) == b"\xff\xd8\xff", "it isn't a JPEG picture")
    return "picture"


# ---- bot files and text ----------------------------------------------------------------------------------

def check_nav(r):
    _need(r.size >= 8, "it's too small to be a navigation file")
    magic, version = r.ints(0, "II")
    _need(magic == 0xFEEDFACE, "it isn't a navigation file")
    _need(1 <= version <= 5, f"it's a version {version} navigation file; the game reads up to 5")
    _need(r.size <= 32 << 20, "it's too big for a navigation file")
    if version < 5:
        return "nav"  # older files: the bots rebuild what they can't read
    # version 5, as the game's bots read it: places, then every area and what it links to
    b, p = r.at(0, r.size), 12

    calls = 0

    def take(fmt):
        nonlocal p, calls
        calls += 1
        if calls & 0xFFF == 0:
            tick()
        n = struct.calcsize("<" + fmt)
        _need(p + n <= len(b), "it ends early")
        v = struct.unpack_from("<" + fmt, b, p)
        p += n
        return v

    places = take("H")[0]
    for _ in range(places):
        n = take("H")[0]  # (read before adding: take() moves p)
        p += n
    areas = take("I")[0]
    _need(areas <= 200000, "it has an impossible number of areas")
    for _ in range(areas):
        take("IB8f")
        for _d in range(4):
            c = take("I")[0]
            _need(c <= areas, "an area has impossible links")
            p += 4 * c
        for _h in range(take("B")[0]):
            take("I3fB")
        n = take("B")[0]
        p += n * 14
        for _e in range(take("I")[0]):
            take("IBIB")
            n = take("B")[0]
            p += n * 5
        place = take("H")[0]
        _need(place <= places, "an area names a place that isn't there")
    _need(p <= len(b), "it ends early")
    # (some real navs carry data after the areas that the bots ignore; that's allowed)
    return "nav"


def check_lsv(r):
    _need(r.size >= 8, "it's too small to be a map knowledge file")
    magic, version = r.ints(0, "II")
    _need(magic == 0x3256534C, "it isn't a map knowledge file")
    _need(1 <= version <= 64, "its version is one the bot brain doesn't read")
    _need(r.size <= 512 << 20, "it's too big for a map knowledge file")
    if version != 6:
        return "knowledge"  # another version: the brain rebuilds it rather than reading it
    # version 6 (goldsrc/lsv.rs): the length must match the counts exactly
    _need(r.size >= 44, "it ends early")
    count, stage, _ci, _cj, pairs, bangs = r.ints(12, "6I")
    _need(count <= 6000 and stage < 5, "it's damaged")
    want = 44 + (count * count + 7) // 8 + pairs * 6 + bangs * 6 + count * 315
    _need(want == r.size, f"it's {r.size} bytes where its counts say {want}")
    return "knowledge"


def check_text(r, as_json=False, limit=16 << 20):
    _need(r.size <= limit, "it's too big for a text file")
    data = r.at(0, r.size)
    if data[:2] in (b"\xff\xfe", b"\xfe\xff"):
        # UTF-16 (the game's own language files)
        try:
            text = data.decode("utf-16")
        except UnicodeDecodeError:
            raise FormatError("it isn't text")
        _need("\0" not in text.rstrip("\0"), "it isn't text")
        return "text"
    _need(b"\0" not in data.rstrip(b"\0"), "it isn't text")
    if as_json:
        text = data.decode("utf-8-sig", "replace").strip()
        try:
            json.loads(text)
        except ValueError:
            # JSON lines (one value a line) count too
            try:
                for line in text.splitlines():
                    if line.strip():
                        json.loads(line)
            except ValueError:
                raise FormatError("it isn't valid JSON")
    return "text"


CHECKS = {".bsp": check_bsp, ".wad": check_wad, ".mdl": check_mdl, ".spr": check_spr, ".wav": check_wav,
          ".mp3": check_mp3, ".bmp": check_bmp, ".tga": check_tga, ".png": check_png, ".jpg": check_jpg,
          ".jpeg": check_jpg, ".nav": check_nav, ".lsv": check_lsv, ".res": check_text, ".txt": check_text,
          ".ini": check_text, ".map": check_text, ".json": lambda r: check_text(r, as_json=True)}


def check(name, f, size):
    """Checks one file (an open binary file of `size` bytes) by its name's extension. Returns
    its kind ("map", "model", ...) or None when there's no check for it; FormatError if bad."""
    low = name.lower()
    ext = low[low.rfind("."):] if "." in low.rsplit("/", 1)[-1] else ""
    fn = CHECKS.get(ext)
    if fn is None:
        return None
    try:
        return fn(Reader(f, size))
    except struct.error:
        raise FormatError("it ends early")
