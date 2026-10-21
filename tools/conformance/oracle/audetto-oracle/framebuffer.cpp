// Framebuffer dump for the conformance oracle. See framebuffer.h.

#include "StdAfx.h"

#include "framebuffer.h"

#include <csignal>
#include <cinttypes>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

#include "CPU.h"
#include "CardManager.h"
#include "Common.h"
#include "Core.h"
#include "Interface.h"
#include "Memory.h"
#include "NTSC.h"
#include "RGBMonitor.h"
#include "Speaker.h"
#include "Video.h"

#include "linux/registryclass.h"

namespace
{
    std::string g_outPrefix;
    std::string g_scenePath;
    int g_videoType = -1; // -1: keep the AppleWin default (VT_DEFAULT = colour TV)
    int g_videoHz = 0;    // 0: keep the default (60)
    int g_rgbCard = 0;

    // ---- SHA-256 (FIPS 180-4), so the oracle needs no crypto library. ----
    struct Sha256
    {
        uint32_t h[8] = {0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19};
        uint8_t buf[64];
        size_t fill = 0;
        uint64_t total = 0;

        static uint32_t rotr(uint32_t x, int n)
        {
            return (x >> n) | (x << (32 - n));
        }

        void block(const uint8_t *p)
        {
            static const uint32_t k[64] = {
                0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
                0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
                0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
                0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
                0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
                0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
                0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
                0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2};
            uint32_t w[64];
            for (int i = 0; i < 16; ++i)
                w[i] = (uint32_t(p[4 * i]) << 24) | (uint32_t(p[4 * i + 1]) << 16) | (uint32_t(p[4 * i + 2]) << 8) |
                       uint32_t(p[4 * i + 3]);
            for (int i = 16; i < 64; ++i)
            {
                const uint32_t s0 = rotr(w[i - 15], 7) ^ rotr(w[i - 15], 18) ^ (w[i - 15] >> 3);
                const uint32_t s1 = rotr(w[i - 2], 17) ^ rotr(w[i - 2], 19) ^ (w[i - 2] >> 10);
                w[i] = w[i - 16] + s0 + w[i - 7] + s1;
            }
            uint32_t a = h[0], b = h[1], c = h[2], d = h[3], e = h[4], f = h[5], g = h[6], hh = h[7];
            for (int i = 0; i < 64; ++i)
            {
                const uint32_t S1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
                const uint32_t ch = (e & f) ^ (~e & g);
                const uint32_t t1 = hh + S1 + ch + k[i] + w[i];
                const uint32_t S0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
                const uint32_t mj = (a & b) ^ (a & c) ^ (b & c);
                const uint32_t t2 = S0 + mj;
                hh = g;
                g = f;
                f = e;
                e = d + t1;
                d = c;
                c = b;
                b = a;
                a = t1 + t2;
            }
            h[0] += a;
            h[1] += b;
            h[2] += c;
            h[3] += d;
            h[4] += e;
            h[5] += f;
            h[6] += g;
            h[7] += hh;
        }

        void update(const uint8_t *p, size_t n)
        {
            total += n;
            while (n)
            {
                const size_t take = std::min(n, sizeof(buf) - fill);
                std::memcpy(buf + fill, p, take);
                fill += take;
                p += take;
                n -= take;
                if (fill == sizeof(buf))
                {
                    block(buf);
                    fill = 0;
                }
            }
        }

        std::string finish()
        {
            const uint64_t bits = total * 8;
            const uint8_t pad = 0x80;
            update(&pad, 1);
            const uint8_t zero = 0;
            while (fill != 56)
                update(&zero, 1);
            uint8_t len[8];
            for (int i = 0; i < 8; ++i)
                len[i] = uint8_t(bits >> (56 - 8 * i));
            update(len, 8);
            char out[65];
            for (int i = 0; i < 8; ++i)
                std::snprintf(out + 8 * i, 9, "%08x", h[i]);
            return out;
        }
    };

    // ---- Scene v1: "main|aux ADDR HEX..." segments and "entry ADDR"; '#' starts a comment. ----
    struct Segment
    {
        bool aux;
        uint16_t addr;
        std::vector<uint8_t> bytes;
    };

    bool loadScene(const std::string &path, std::vector<Segment> &segs, int &entry)
    {
        std::ifstream in(path);
        if (!in)
        {
            std::fprintf(stderr, "[oracle] cannot read scene %s\n", path.c_str());
            return false;
        }
        entry = -1;
        std::string line;
        while (std::getline(in, line))
        {
            const size_t hash = line.find('#');
            if (hash != std::string::npos)
                line.erase(hash);
            std::istringstream ls(line);
            std::string kind;
            if (!(ls >> kind))
                continue;
            if (kind == "main" || kind == "aux")
            {
                std::string addr;
                ls >> addr;
                Segment s{kind == "aux", static_cast<uint16_t>(std::strtoul(addr.c_str(), nullptr, 16)), {}};
                std::string byte;
                while (ls >> byte)
                    s.bytes.push_back(static_cast<uint8_t>(std::strtoul(byte.c_str(), nullptr, 16)));
                segs.push_back(std::move(s));
            }
            else if (kind == "entry")
            {
                std::string addr;
                ls >> addr;
                entry = static_cast<int>(std::strtoul(addr.c_str(), nullptr, 16));
            }
            // "scene", "name", "model": informational
        }
        return entry >= 0;
    }

    // Writes both the backing store and the 'mem' cache, so a later paging change cannot undo it.
    void poke(bool aux, uint16_t addr, uint8_t value)
    {
        LPBYTE bank = MemGetBankPtr(aux ? 1 : 0, false);
        bank[addr] = value;
        LPBYTE view = aux ? MemGetAuxPtr(addr) : MemGetMainPtr(addr);
        *view = value;
    }
} // namespace

namespace fbdump
{
    bool parseArg(int argc, char **argv, int &i)
    {
        const std::string arg = argv[i];
        if (i + 1 >= argc)
            return false;
        if (arg == "--fb-out")
            g_outPrefix = argv[++i];
        else if (arg == "--scene")
            g_scenePath = argv[++i];
        else if (arg == "--video-type")
            g_videoType = std::atoi(argv[++i]);
        else if (arg == "--video-hz")
            g_videoHz = std::atoi(argv[++i]);
        else if (arg == "--rgb-card")
            g_rgbCard = std::atoi(argv[++i]);
        else
            return false;
        return true;
    }

    bool enabled()
    {
        return !g_outPrefix.empty();
    }

    void configureRegistry(Registry &registry)
    {
        if (g_videoType >= 0)
            registry.putDWord(REG_CONFIG, REGVALUE_VIDEO_MODE, static_cast<uint32_t>(g_videoType));
        if (g_videoHz == 50)
            registry.putDWord(REG_CONFIG, REGVALUE_VIDEO_REFRESH_RATE, static_cast<uint32_t>(VR_50HZ));
        else if (g_videoHz == 60)
            registry.putDWord(REG_CONFIG, REGVALUE_VIDEO_REFRESH_RATE, static_cast<uint32_t>(VR_60HZ));
    }

    int run(uint64_t frames)
    {
        RGB_SetVideocard(static_cast<RGB_Videocard_e>(g_rgbCard));
        // The card was chosen after the video type was loaded: rebuild the palette and the scanner tables.
        GetVideo().VideoReinitialize(false);

        if (!g_scenePath.empty())
        {
            std::vector<Segment> segs;
            int entry = -1;
            if (!loadScene(g_scenePath, segs, entry))
                return 1;
            for (const Segment &s : segs)
                for (size_t k = 0; k < s.bytes.size(); ++k)
                    poke(s.aux, static_cast<uint16_t>(s.addr + k), s.bytes[k]);
            regs.pc = static_cast<WORD>(entry);
        }

        if (const char *watch = std::getenv("FB_WATCH")) // "x,y" top-down: print its address, then SIGTRAP for gdb
        {
            unsigned wx = 0, wy = 0;
            std::sscanf(watch, "%u,%u", &wx, &wy);
            const UINT fh0 = GetVideo().GetFrameBufferHeight(), fw0 = GetVideo().GetFrameBufferWidth();
            const uint32_t *p0 = reinterpret_cast<const uint32_t *>(GetVideo().GetFrameBuffer());
            std::fprintf(stderr, "[fb] watch %p\n", static_cast<const void *>(p0 + (fh0 - 1 - wy) * fw0 + wx));
            std::raise(SIGTRAP);
        }

        const UINT cyclesPerFrame = NTSC_GetCyclesPerFrame();
        const uint64_t start = g_nCumulativeCycles;
        const uint64_t end = start + frames * cyclesPerFrame;
        while (g_nCumulativeCycles < end)
        {
            // bVideoUpdate = true: NTSC_VideoUpdateCycles() after every opcode, like the real frontends.
            const uint32_t executed = CpuExecute(static_cast<uint32_t>(end - g_nCumulativeCycles), true);
            GetCardMgr().Update(executed);
            SpkrUpdate(executed);
            g_dwCyclesThisFrame = (g_dwCyclesThisFrame + executed) % cyclesPerFrame;
        }

        if (std::getenv("FB_DEBUG"))
        {
            uint16_t v = 0, h = 0;
            NTSC_GetVideoVertHorzForDebugger(v, h);
            std::fprintf(stderr, "[fb] scanner vert %u horz %u, cyclesThisFrame %u\n", v, h,
                static_cast<unsigned>(g_dwCyclesThisFrame));
            std::fprintf(stderr, "[fb] video mode flags %08X, type %d, style %d, hz %d\n",
                static_cast<unsigned>(GetVideo().GetVideoMode()), static_cast<int>(GetVideo().GetVideoType()),
                static_cast<int>(GetVideo().GetVideoStyle()), static_cast<int>(GetVideo().GetVideoRefreshRate()));
            for (int aux = 0; aux < 2; ++aux)
            {
                std::fprintf(stderr, "[fb] %s $0400:", aux ? "aux" : "main");
                for (int k = 0; k < 16; ++k)
                    std::fprintf(stderr, " %02X", MemGetBankPtr(aux, false)[0x400 + k]);
                std::fprintf(stderr, "  view:");
                for (int k = 0; k < 16; ++k)
                    std::fprintf(stderr, " %02X", (aux ? MemGetAuxPtr(0x400) : MemGetMainPtr(0x400))[k]);
                std::fprintf(stderr, "\n");
            }
        }
        Video &video = GetVideo();
        const UINT fw = video.GetFrameBufferWidth();
        const UINT fh = video.GetFrameBufferHeight();
        const UINT bw = video.GetFrameBufferBorderWidth();
        const UINT bh = video.GetFrameBufferBorderHeight();
        const UINT vw = video.GetFrameBufferBorderlessWidth();
        const UINT vh = video.GetFrameBufferBorderlessHeight();
        const uint32_t *fb = reinterpret_cast<const uint32_t *>(video.GetFrameBuffer());

        // 0xAARRGGBB words -> R,G,B,A bytes. The AppleWin framebuffer is a Windows DIB, bottom-up
        // (NTSC_VideoInit maps scan line y to row FrameBufferHeight-1-y), so rows are flipped here.
        std::vector<uint8_t> full(static_cast<size_t>(fw) * fh * 4);
        for (size_t y = 0; y < fh; ++y)
            for (size_t x = 0; x < fw; ++x)
            {
                const uint32_t c = fb[(fh - 1 - y) * fw + x];
                uint8_t *d = &full[4 * (y * fw + x)];
                d[0] = uint8_t(c >> 16);
                d[1] = uint8_t(c >> 8);
                d[2] = uint8_t(c);
                d[3] = uint8_t(c >> 24);
            }
        std::vector<uint8_t> visible;
        visible.reserve(static_cast<size_t>(vw) * vh * 4);
        for (UINT y = 0; y < vh; ++y)
        {
            const uint8_t *row = &full[(static_cast<size_t>(y + bh) * fw + bw) * 4];
            visible.insert(visible.end(), row, row + static_cast<size_t>(vw) * 4);
        }

        Sha256 shaVisible, shaFull;
        shaVisible.update(visible.data(), visible.size());
        shaFull.update(full.data(), full.size());

        const std::string rgbaPath = g_outPrefix + ".rgba";
        FILE *f = std::fopen(rgbaPath.c_str(), "wb");
        if (!f)
        {
            std::perror(rgbaPath.c_str());
            return 1;
        }
        std::fwrite(full.data(), 1, full.size(), f);
        std::fclose(f);

        std::printf("fb_size=%ux%u\n", fw, fh);
        std::printf("fb_border=%ux%u\n", bw, bh);
        std::printf("fb_visible=%ux%u\n", vw, vh);
        std::printf("fb_sha256_visible=%s\n", shaVisible.finish().c_str());
        std::printf("fb_sha256_full=%s\n", shaFull.finish().c_str());
        std::printf("fb_video_type=%d\n", static_cast<int>(video.GetVideoType()));
        std::printf("fb_cycles=%" PRIu64 "\n", static_cast<uint64_t>(g_nCumulativeCycles));
        std::printf("fb_frames=%" PRIu64 "\n", frames);
        std::printf("fb_pc=%04X\n", static_cast<unsigned>(regs.pc));
        return 0;
    }
} // namespace fbdump
