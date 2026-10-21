// Audio side of the conformance oracle. See audio.h and tools/conformance/oracle/README.md ("Audio").
//
// What it adds to the headless oracle:
//  * an event log (audio-events v1): card-page accesses, AY register accesses, IRQ line transitions,
//    speaker toggles, SSI263 phoneme start/end, each with the 6502 cycle (relative to the start of the run);
//  * raw PCM dumps: Mockingboard mix (44.1 kHz, stereo s16le), speaker (44.1 kHz, mono s16le) and the
//    SSI263 voices (22.05 kHz, mono s16le);
//  * a deterministic "virtual DirectSound" so that the sound code that is normally paced by a real audio
//    device (MockingboardCardManager::UpdateSoundBuffer, SSI263::Update) runs on emulated time;
//  * a loader for small 6502 test programs, so both emulators start from the same RAM and registers.

#include "StdAfx.h"

#include "audio.h"

#include <algorithm>
#include <cinttypes>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include "CPU.h"
#include "CardManager.h"
#include "Core.h"
#include "Memory.h"
#include "Mockingboard.h"
#include "MockingboardDefs.h"
#include "SoundBuffer.h"
#include "audio_hook.h"

namespace
{
    struct Options
    {
        bool enabled = false;
        std::string card;
        UINT slot = 4;
        std::string eventsPath, mbPcmPath, spkrPcmPath, ssiPcmPrefix, program;
        uint32_t loadAddr = 0x0300, startPc = 0x0300;
        bool havePc = false;
        uint32_t slice = 0;
        std::vector<std::pair<int, AY891xType>> aySockets;
        std::vector<std::pair<int, SSI263Type>> ssiSockets;
        bool bad6522[2] = {false, false};
        int sc01 = -1; // -1 = leave the default
    } g_opt;

    FILE *g_events = nullptr;
    FILE *g_mbPcm = nullptr;
    FILE *g_spkrPcm = nullptr;
    FILE *g_ssiPcm[2] = {nullptr, nullptr}; // per SSI263 device
    uint64_t g_startCycle = 0;
    uint32_t g_irqState = 0;
    uint64_t g_eventLines = 0, g_mbFrames = 0, g_spkrFrames = 0;

    bool parseHex(const char *s, uint32_t &out)
    {
        char *end = nullptr;
        out = static_cast<uint32_t>(std::strtoul(s, &end, 16));
        return end && *end == 0 && end != s;
    }

    bool parseAyType(const std::string &s, AY891xType &t)
    {
        if (s == "empty")
            t = AY_Empty;
        else if (s == "ay8910")
            t = AY_3_8910;
        else if (s == "ay8912")
            t = AY_3_8912;
        else if (s == "ay8913")
            t = AY_3_8913;
        else if (s == "ym2149")
            t = YM2149F;
        else
            return false;
        return true;
    }

    bool parseSsiType(const std::string &s, SSI263Type &t)
    {
        if (s == "empty")
            t = SSI263Empty;
        else if (s == "ssi263p")
            t = SSI263P;
        else if (s == "ssi263ap")
            t = SSI263AP;
        else
            return false;
        return true;
    }

    const char *irqName(int src)
    {
        switch (src)
        {
        case IS_6522:
            return "6522";
        case IS_SPEECH:
            return "SPEECH";
        case IS_SSC:
            return "SSC";
        case IS_MOUSE:
            return "MOUSE";
        default:
            return "OTHER";
        }
    }

    void onEvent(int kind, int a, int b, int c, int d)
    {
        if (!g_events)
            return;
        const uint64_t cycle = static_cast<uint64_t>(g_nCumulativeCycles) - g_startCycle;
        switch (kind)
        {
        case OAE_IO_W:
            std::fprintf(g_events, "%" PRIu64 ",IOW,%d,%02X,%02X\n", cycle, a, b & 0xFF, c & 0xFF);
            break;
        case OAE_IO_R:
            std::fprintf(g_events, "%" PRIu64 ",IOR,%d,%02X,%02X\n", cycle, a, b & 0xFF, c & 0xFF);
            break;
        case OAE_AY_W:
            std::fprintf(g_events, "%" PRIu64 ",AYW,%d,%d,%d,%X,%02X\n", cycle, a, (b >> 4) & 0xF, b & 0xF, c & 0xF, d & 0xFF);
            break;
        case OAE_AY_R:
            std::fprintf(g_events, "%" PRIu64 ",AYR,%d,%d,%d,%X,%02X\n", cycle, a, (b >> 4) & 0xF, b & 0xF, c & 0xF, d & 0xFF);
            break;
        case OAE_IRQ:
        {
            const uint32_t bit = 1u << a;
            const bool level = b != 0;
            if (level == ((g_irqState & bit) != 0))
                return; // no transition
            g_irqState = level ? (g_irqState | bit) : (g_irqState & ~bit);
            std::fprintf(g_events, "%" PRIu64 ",IRQ,%s,%d\n", cycle, irqName(a), level ? 1 : 0);
            break;
        }
        case OAE_SPK_TOGGLE:
            std::fprintf(g_events, "%" PRIu64 ",SPK\n", cycle);
            break;
        case OAE_PHONEME_START:
            std::fprintf(g_events, "%" PRIu64 ",PHON,%d,%d,%02X,%d\n", cycle, a, b, c & 0xFF, d);
            break;
        case OAE_PHONEME_END:
            std::fprintf(g_events, "%" PRIu64 ",PHEND,%d,%d\n", cycle, a, b);
            break;
        case OAE_PHASOR_IO:
            std::fprintf(g_events, "%" PRIu64 ",PIO,%d,%02X,%d\n", cycle, a, b & 0xFF, c ? 1 : 0);
            break;
        default:
            return;
        }
        ++g_eventLines;
    }

    void onMbPcm(const short *frames, unsigned n)
    {
        if (g_mbPcm)
            std::fwrite(frames, sizeof(short) * 2, n, g_mbPcm);
        g_mbFrames += n;
    }

    void onSpkrPcm(const short *frames, unsigned n, int channels)
    {
        if (g_spkrPcm)
        {
            for (unsigned i = 0; i < n; ++i)
                std::fwrite(&frames[i * channels], sizeof(short), 1, g_spkrPcm);
        }
        g_spkrFrames += n;
    }

    void onSsiPcm(int slot, int device, const short *mono, unsigned n)
    {
        (void)slot;
        if (device >= 0 && device < 2 && g_ssiPcm[device])
            std::fwrite(mono, sizeof(short), n, g_ssiPcm[device]);
    }

    // A DirectSound-like ring buffer driven by emulated time instead of a sound device.
    //
    // Play cursor: advances with g_nCumulativeCycles at the buffer's sample rate, never past the write
    // cursor (an underrun stalls it, like LinuxSoundBuffer). Write cursor: end of the data written so far.
    // The Mockingboard buffer starts pre-filled with 3/8 of silence: inside the 1/4..1/2 band where the
    // AppleWin feedback (m_numSamplesError) is zero, so the generated PCM is not time-stretched at
    // start-up. The SSI263 buffers start empty: SSI263::Update pre-fills 25% itself on its first call.
    class VirtualSoundBuffer : public SoundBuffer
    {
    public:
        // `leadEighths`: initial pre-fill of silence, in eighths of the buffer.
        VirtualSoundBuffer(uint32_t bufferSize, uint32_t sampleRate, int channels, unsigned leadEighths)
            : myData(bufferSize)
            , mySampleRate(sampleRate)
            , myBytesPerFrame(sizeof(short) * channels)
        {
            const size_t frames = bufferSize / myBytesPerFrame;
            myWritePos = (frames * leadEighths / 8) * myBytesPerFrame;
            myAvailable = myWritePos;
            myLastCycle = g_nCumulativeCycles;
        }

        HRESULT SetCurrentPosition(DWORD) override { return DS_OK; }

        HRESULT GetCurrentPosition(LPDWORD play, LPDWORD write) override
        {
            advance();
            *play = static_cast<DWORD>(myPlayPos);
            *write = static_cast<DWORD>(myWritePos);
            if (std::getenv("ORACLE_DS_DEBUG"))
                std::fprintf(stderr, "[ds %zu] cyc=%" PRIu64 " play=%zu write=%zu avail=%zu\n", mySampleRate,
                    static_cast<uint64_t>(g_nCumulativeCycles), myPlayPos, myWritePos, myAvailable);
            return DS_OK;
        }

        HRESULT Lock(DWORD offset, DWORD bytes, LPVOID *p1, DWORD *n1, LPVOID *p2, DWORD *n2, DWORD flags) override
        {
            myEntireLock = (flags & DSBLOCK_ENTIREBUFFER) != 0;
            if (myEntireLock)
            {
                *p1 = myData.data();
                *n1 = static_cast<DWORD>(myData.size());
                if (p2 && n2)
                {
                    *p2 = nullptr;
                    *n2 = 0;
                }
                return DS_OK;
            }
            offset %= myData.size();
            const DWORD first = static_cast<DWORD>(std::min<size_t>(myData.size() - offset, bytes));
            *p1 = myData.data() + offset;
            *n1 = first;
            if (p2 && n2)
            {
                if (first < bytes)
                {
                    *p2 = myData.data();
                    *n2 = std::min<DWORD>(offset, bytes - first);
                }
                else
                {
                    *p2 = nullptr;
                    *n2 = 0;
                }
            }
            return DS_OK;
        }

        HRESULT Unlock(LPVOID, DWORD n1, LPVOID, DWORD n2) override
        {
            advance();
            if (myEntireLock)
                return DS_OK; // DSZeroVoiceBuffer clears the whole buffer: the cursors do not move
            const size_t written = static_cast<size_t>(n1) + n2;
            myWritePos = (myWritePos + written) % myData.size();
            myAvailable = std::min(myData.size(), myAvailable + written);
            return DS_OK;
        }

        HRESULT Stop() override
        {
            advance();
            myStatus &= ~(DSBSTATUS_PLAYING | DSBSTATUS_LOOPING);
            return DS_OK;
        }

        HRESULT Play(DWORD, DWORD, DWORD flags) override
        {
            advance();
            myStatus |= DSBSTATUS_PLAYING;
            if (flags & DSBPLAY_LOOPING)
                myStatus |= DSBSTATUS_LOOPING;
            return DS_OK;
        }

        HRESULT SetVolume(LONG v) override
        {
            myVolume = v;
            return DS_OK;
        }
        HRESULT GetVolume(LONG *v) override
        {
            *v = myVolume;
            return DS_OK;
        }
        HRESULT GetStatus(LPDWORD s) override
        {
            *s = myStatus;
            return DS_OK;
        }
        HRESULT Restore() override { return DS_OK; }

    private:
        void advance()
        {
            const uint64_t now = g_nCumulativeCycles;
            if (now < myLastCycle)
                myLastCycle = now;
            if (myStatus & DSBSTATUS_PLAYING)
            {
                const double frames = myFraction + static_cast<double>(now - myLastCycle) * mySampleRate / g_fCurrentCLK6502;
                const uint64_t whole = static_cast<uint64_t>(frames);
                myFraction = frames - static_cast<double>(whole);
                const size_t consumed = std::min<size_t>(whole * myBytesPerFrame, myAvailable);
                myPlayPos = (myPlayPos + consumed) % myData.size();
                myAvailable -= consumed;
            }
            myLastCycle = now;
        }

        std::vector<uint8_t> myData;
        size_t mySampleRate, myBytesPerFrame;
        size_t myPlayPos = 0, myWritePos = 0, myAvailable = 0;
        uint64_t myLastCycle = 0;
        double myFraction = 0.0;
        DWORD myStatus = 0;
        bool myEntireLock = false;
        LONG myVolume = 0;
    };

    FILE *openOut(const std::string &path)
    {
        FILE *f = std::fopen(path.c_str(), "wb");
        if (!f)
            std::perror(path.c_str());
        return f;
    }
} // namespace

namespace audio_oracle
{
    bool parseArg(int &i, int argc, char **argv)
    {
        const std::string arg = argv[i];
        if (arg.rfind("--audio-", 0) != 0)
            return false;
        if (i + 1 >= argc)
            return false;
        const std::string val = argv[++i];
        g_opt.enabled = true;
        if (arg == "--audio-card")
            g_opt.card = val;
        else if (arg == "--audio-slot")
            g_opt.slot = static_cast<UINT>(std::atoi(val.c_str()));
        else if (arg == "--audio-events")
            g_opt.eventsPath = val;
        else if (arg == "--audio-mb-pcm")
            g_opt.mbPcmPath = val;
        else if (arg == "--audio-spkr-pcm")
            g_opt.spkrPcmPath = val;
        else if (arg == "--audio-ssi-pcm")
            g_opt.ssiPcmPrefix = val;
        else if (arg == "--audio-program")
            g_opt.program = val;
        else if (arg == "--audio-load")
        {
            if (!parseHex(val.c_str(), g_opt.loadAddr))
                return false;
            if (!g_opt.havePc)
                g_opt.startPc = g_opt.loadAddr;
        }
        else if (arg == "--audio-pc")
        {
            g_opt.havePc = true;
            return parseHex(val.c_str(), g_opt.startPc);
        }
        else if (arg == "--audio-slice")
            g_opt.slice = static_cast<uint32_t>(std::strtoul(val.c_str(), nullptr, 10));
        else if (arg == "--audio-ay-socket")
        {
            // N=type
            const auto eq = val.find('=');
            AY891xType t;
            if (eq == std::string::npos || !parseAyType(val.substr(eq + 1), t))
                return false;
            g_opt.aySockets.emplace_back(std::atoi(val.c_str()), t);
        }
        else if (arg == "--audio-ssi263")
        {
            const auto eq = val.find('=');
            SSI263Type t;
            if (eq == std::string::npos || !parseSsiType(val.substr(eq + 1), t))
                return false;
            g_opt.ssiSockets.emplace_back(std::atoi(val.c_str()), t);
        }
        else if (arg == "--audio-bad6522")
        {
            if (val == "a")
                g_opt.bad6522[0] = true;
            else if (val == "b")
                g_opt.bad6522[1] = true;
            else
                return false;
        }
        else if (arg == "--audio-sc01")
            g_opt.sc01 = (val == "on") ? 1 : 0;
        else
            return false;
        return true;
    }

    bool enabled() { return g_opt.enabled; }

    SS_CARDTYPE cardTypeFor(UINT slot, SS_CARDTYPE dflt)
    {
        if (!g_opt.enabled || g_opt.card.empty() || slot != g_opt.slot)
            return dflt;
        if (g_opt.card == "mockingboard")
            return CT_MockingboardC;
        if (g_opt.card == "phasor")
            return CT_Phasor;
        if (g_opt.card == "megaaudio")
            return CT_MegaAudio;
        if (g_opt.card == "sdmusic")
            return CT_SDMusic;
        return dflt;
    }

    uint32_t slice(uint32_t dflt) { return (g_opt.enabled && g_opt.slice) ? g_opt.slice : dflt; }

    std::shared_ptr<SoundBuffer> createSoundBuffer(uint32_t bufferSize, uint32_t sampleRate, int channels, const char *name)
    {
        if (!g_opt.enabled)
            return nullptr;
        // The speaker keeps no voice (its PCM is taken from the speaker queue, see audetto-audio.patch):
        // a real voice would add the speaker's cycle feedback (g_nCpuCyclesFeedback) to the CPU clock.
        if (name && std::strcmp(name, "Spkr") == 0)
            return nullptr;
        const unsigned lead = (name && std::strcmp(name, "SSI263") == 0) ? 0 : 3;
        return std::make_shared<VirtualSoundBuffer>(bufferSize, sampleRate, channels, lead);
    }

    void begin()
    {
        if (!g_opt.enabled)
            return;

        g_startCycle = g_nCumulativeCycles;

        if (!g_opt.card.empty() && g_opt.slot >= SLOT1 && g_opt.slot <= SLOT7 && GetCardMgr().GetRef(g_opt.slot).QueryType() != CT_Empty)
        {
            MockingboardCard &mb = dynamic_cast<MockingboardCard &>(GetCardMgr().GetRef(g_opt.slot));
            if (g_opt.bad6522[0])
                mb.UseBad6522A();
            if (g_opt.bad6522[1])
                mb.UseBad6522B();
            for (const auto &s : g_opt.aySockets)
                mb.SetSocketAY891x(static_cast<BYTE>(s.first), s.second);
            for (const auto &s : g_opt.ssiSockets)
                mb.SetSocketSSI263(static_cast<BYTE>(s.first), s.second);
            if (g_opt.sc01 >= 0)
                mb.SetSocketSC01(g_opt.sc01 ? SC01 : SSI263Empty);
        }

        if (!g_opt.program.empty())
        {
            FILE *f = std::fopen(g_opt.program.c_str(), "rb");
            if (!f)
            {
                std::perror(g_opt.program.c_str());
                std::exit(1);
            }
            std::vector<uint8_t> bytes;
            uint8_t chunk[4096];
            size_t n;
            while ((n = std::fread(chunk, 1, sizeof(chunk), f)) > 0)
                bytes.insert(bytes.end(), chunk, chunk + n);
            std::fclose(f);
            for (size_t k = 0; k < bytes.size(); ++k)
                *MemGetMainPtr(static_cast<WORD>(g_opt.loadAddr + k)) = bytes[k];
            regs.pc = static_cast<WORD>(g_opt.startPc);
        }

        if (!g_opt.eventsPath.empty())
        {
            g_events = openOut(g_opt.eventsPath);
            if (g_events)
            {
                std::fprintf(g_events, "#audio_events=v1\n#oracle=audetto-oracle\n#card=%s\n#slot=%u\n", g_opt.card.c_str(), g_opt.slot);
                std::fprintf(g_events, "#start_cycle=%" PRIu64 "\n", g_startCycle);
            }
        }
        if (!g_opt.mbPcmPath.empty())
            g_mbPcm = openOut(g_opt.mbPcmPath);
        if (!g_opt.spkrPcmPath.empty())
            g_spkrPcm = openOut(g_opt.spkrPcmPath);
        if (!g_opt.ssiPcmPrefix.empty())
        {
            for (int d = 0; d < 2; ++d)
                g_ssiPcm[d] = openOut(g_opt.ssiPcmPrefix + "." + std::to_string(d) + ".pcm");
        }

        g_pfnOracleAudioEvent = onEvent;
        g_pfnOracleMbPcm = onMbPcm;
        g_pfnOracleSpkrPcm = onSpkrPcm;
        g_pfnOracleSsiPcm = onSsiPcm;

        // CommonInitialisation ran with "no audio"; turn the sound code on now (cards and the speaker voice
        // were already set up: the speaker has no voice, the Mockingboard / SSI263 voices are created lazily
        // on the first update and get a VirtualSoundBuffer).
        g_bDisableDirectSound = false;
        g_bDisableDirectSoundMockingboard = false;
    }

    void end()
    {
        g_pfnOracleAudioEvent = nullptr;
        g_pfnOracleMbPcm = nullptr;
        g_pfnOracleSpkrPcm = nullptr;
        g_pfnOracleSsiPcm = nullptr;
        for (FILE *f : {g_events, g_mbPcm, g_spkrPcm, g_ssiPcm[0], g_ssiPcm[1]})
        {
            if (f)
                std::fclose(f);
        }
        g_events = g_mbPcm = g_spkrPcm = nullptr;
        g_ssiPcm[0] = g_ssiPcm[1] = nullptr;
        if (g_opt.enabled)
            std::fprintf(stderr, "[oracle] audio: %" PRIu64 " event lines, %" PRIu64 " MB frames, %" PRIu64 " speaker frames\n",
                g_eventLines, g_mbFrames, g_spkrFrames);
    }
} // namespace audio_oracle
