// Headless conformance oracle built on the audetto/AppleWin core.
//
// Runs the emulated machine for N cycles (or N frames) with no window, no audio, no joystick and no
// wall-clock pacing, and writes a Trace in "v1" format (see tools/conformance/oracle/README.md):
//
//   #key=value            metadata lines
//   cycle,pc,op,a,x,y,sp,p
//   <decimal>,<PC:4 hex>,<2 hex>,...   one line per instruction, written BEFORE it executes
//
// The per-instruction hook is g_pfnCpuTraceHook, added by tools/conformance/oracle/audetto-trace.patch.
// This file is copied into <fork>/source/frontends/oracle/ by build-audetto-oracle.sh.

#include "StdAfx.h"

#include <cinttypes>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <string>
#include <vector>

#include "CPU.h"
#include "CardManager.h"
#include "Core.h"
#include "Interface.h"
#include "Log.h"
#include "Memory.h"
#include "NTSC.h"
#include "Registry.h"
#include "SaveState.h"
#include "Speaker.h"

#include "audio.h"
#include "linux/context.h"
#include "linux/paddle.h"
#include "linux/registryclass.h"
#include "frontends/common2/commoncontext.h"
#include "frontends/common2/gnuframe.h"
#include "frontends/common2/programoptions.h"
#include "frontends/common2/ptreeregistry.h"

namespace
{
    FILE *g_out = nullptr;
    uint64_t g_limit = 0; // trace lines are written only while cycle < g_limit
    uint64_t g_lines = 0;

    void traceHook(uint64_t cycle, uint16_t pc, uint8_t op, uint8_t a, uint8_t x, uint8_t y, uint8_t sp, uint8_t p)
    {
        if (cycle >= g_limit)
            return;
        std::fprintf(
            g_out, "%" PRIu64 ",%04X,%02X,%02X,%02X,%02X,%02X,%02X\n", cycle, pc, op, a, x, y, sp, p);
        ++g_lines;
    }

    class OracleFrame : public common2::GNUFrame
    {
    public:
        explicit OracleFrame(const common2::EmulatorOptions &options)
            : common2::GNUFrame(options)
        {
        }

        void VideoPresentScreen() override {}

        int FrameMessageBox(LPCSTR lpText, LPCSTR lpCaption, UINT) override
        {
            std::fprintf(stderr, "[oracle] MessageBox: %s: %s\n", lpCaption, lpText);
            return IDOK;
        }

        std::shared_ptr<SoundBuffer> CreateSoundBuffer(uint32_t size, uint32_t rate, int channels, const char *name) override
        {
            return audio_oracle::createSoundBuffer(size, rate, channels, name); // nullptr unless --audio-* is given
        }

        // Same batching as CommonFrame::Execute (1 ms slices), but with no video rendering.
        void RunUntil(const uint64_t endCycle)
        {
            const UINT clksPerFrame = NTSC_GetCyclesPerFrame();
            const uint32_t slice = audio_oracle::slice(static_cast<uint32_t>(g_fCurrentCLK6502 * (1.0 / 1000.0)));
            while (g_nCumulativeCycles < endCycle)
            {
                const uint32_t executed = CpuExecute(slice, false);
                GetCardMgr().Update(executed);
                SpkrUpdate(executed);
                g_dwCyclesThisFrame = (g_dwCyclesThisFrame + executed) % clksPerFrame;
            }
        }
    };

    bool parseModel(const std::string &name, eApple2Type &type)
    {
        if (name == "apple2")
            type = A2TYPE_APPLE2;
        else if (name == "apple2p" || name == "apple2plus")
            type = A2TYPE_APPLE2PLUS;
        else if (name == "apple2jp" || name == "apple2jplus")
            type = A2TYPE_APPLE2JPLUS;
        else if (name == "apple2e")
            type = A2TYPE_APPLE2E;
        else if (name == "apple2ee" || name == "apple2e-enhanced")
            type = A2TYPE_APPLE2EENHANCED;
        else
            return false;
        return true;
    }

    void usage()
    {
        std::fprintf(
            stderr,
            "usage: audetto-oracle --model <apple2|apple2p|apple2jp|apple2e|apple2ee>\n"
            "                      (--cycles N | --frames N) --out FILE\n"
            "                      [--disk IMAGE] [--state FILE.aws.yaml] [--memclear 0..7]\n"
            "                      [--cards disk2|none] [--meta key=value]...\n");
    }
} // namespace

int main(int argc, char **argv)
{
    std::string modelName, outPath, disk, state, input = "none";
    uint64_t cycles = 0, frames = 0;
    int memclear = -100;
    std::string cards = "disk2";
    std::vector<std::string> meta;

    for (int i = 1; i < argc; ++i)
    {
        const std::string arg = argv[i];
        const bool hasValue = i + 1 < argc;
        if (arg == "--model" && hasValue)
            modelName = argv[++i];
        else if (arg == "--cycles" && hasValue)
            cycles = std::strtoull(argv[++i], nullptr, 10);
        else if (arg == "--frames" && hasValue)
            frames = std::strtoull(argv[++i], nullptr, 10);
        else if (arg == "--out" && hasValue)
            outPath = argv[++i];
        else if (arg == "--disk" && hasValue)
            disk = argv[++i];
        else if (arg == "--state" && hasValue)
            state = argv[++i];
        else if (arg == "--memclear" && hasValue)
            memclear = std::atoi(argv[++i]);
        else if (arg == "--cards" && hasValue)
            cards = argv[++i];
        else if (arg == "--meta" && hasValue)
            meta.emplace_back(argv[++i]);
        else if (audio_oracle::parseArg(i, argc, argv))
        {
        }
        else
        {
            usage();
            return 2;
        }
    }

    eApple2Type model = A2TYPE_APPLE2EENHANCED;
    if (outPath.empty() || (cycles == 0) == (frames == 0) || (state.empty() && !parseModel(modelName, model)) ||
        (cards != "disk2" && cards != "none"))
    {
        usage();
        return 2;
    }

    try
    {
        // No wall-clock randomness in the power-on RAM (patch), and a fixed rand() seed for the disk code.
        g_bOracleDeterministic = true;
        std::srand(1);

        common2::EmulatorOptions options;
        options.autoBoot = true;
        options.fixedSpeed = true;
        options.syncWithTimer = false;
        options.noVideoUpdate = true;
        options.noAudio = true; // g_bDisableDirectSound*, no audio device is ever opened
        options.headless = true;
        options.disk1 = disk;
        if (memclear != -100)
            options.memclear = memclear;
        if (!state.empty())
        {
            options.snapshotFilename = state;
            options.loadSnapshot = true;
        }

        // Fresh in-memory registry: no config file is read or written, so every run starts from the same
        // defaults, whatever the user's ~/.config/applewin contains.
        const auto registry = std::make_shared<common2::PTreeRegistry>();
        const RegistryContext registryContext(registry);
        registry->putDWord(REG_CONFIG, REGVALUE_APPLE2_TYPE, static_cast<uint32_t>(model));
        registry->putDWord(REG_CONFIG, REGVALUE_CPU_TYPE, static_cast<uint32_t>(ProbeMainCpuDefault(model)));
        for (UINT slot = SLOT1; slot <= SLOT7; ++slot)
        {
            const SS_CARDTYPE type = (slot == SLOT6 && cards == "disk2") ? CT_Disk2 : CT_Empty;
            RegSetConfigSlotNewCardType(slot, audio_oracle::cardTypeFor(slot, type));
        }

        const LoggerContext loggerContext(false);
        const auto frame = std::make_shared<OracleFrame>(options);
        // The default Paddle reports a centred, button-less joystick (no real device).
        const common2::CommonInitialisation init(frame, std::make_shared<Paddle>(), options);

        g_bDisableDirectSound = true;
        g_bDisableDirectSoundMockingboard = true;
        audio_oracle::begin();

        const uint64_t start = g_nCumulativeCycles;
        const uint64_t total = cycles ? cycles : frames * NTSC_GetCyclesPerFrame();
        g_limit = start + total;

        g_out = std::fopen(outPath.c_str(), "wb");
        if (!g_out)
        {
            std::perror(outPath.c_str());
            return 1;
        }
        static char buffer[1 << 20];
        std::setvbuf(g_out, buffer, _IOFBF, sizeof(buffer));

        std::fprintf(g_out, "#trace_format=v1\n#oracle=audetto-oracle\n");
        std::fprintf(g_out, "#model=%s\n", modelName.empty() ? "(from save-state)" : modelName.c_str());
        std::fprintf(g_out, "#cpu=%s\n", GetMainCpu() == CPU_65C02 ? "65C02" : "6502");
        if (frames)
            std::fprintf(g_out, "#frames=%" PRIu64 "\n", frames);
        std::fprintf(g_out, "#cycles=%" PRIu64 "\n", total);
        std::fprintf(g_out, "#start_cycle=%" PRIu64 "\n", start);
        std::fprintf(g_out, "#memclear=%d\n", g_nMemoryClearType);
        std::fprintf(g_out, "#ram_init=deterministic (xx28,xx29,xx68,xx69 of pages 00-BF = 00; $4E,$4F = 20)\n");
        std::fprintf(g_out, "#cards=%s\n", cards.c_str());
        std::fprintf(g_out, "#input=%s\n", input.c_str());
        std::fprintf(g_out, "#disk=%s\n", disk.empty() ? "none" : disk.c_str());
        std::fprintf(g_out, "#state=%s\n", state.empty() ? "none" : state.c_str());
        for (const std::string &m : meta)
            std::fprintf(g_out, "#%s\n", m.c_str());
        std::fprintf(g_out, "cycle,pc,op,a,x,y,sp,p\n");

        g_pfnCpuTraceHook = traceHook;
        frame->RunUntil(g_limit);
        g_pfnCpuTraceHook = nullptr;

        std::fclose(g_out);
        audio_oracle::end();
        std::fprintf(stderr, "[oracle] %" PRIu64 " instructions, final cycle %" PRIu64 "\n", g_lines,
            static_cast<uint64_t>(g_nCumulativeCycles));
    }
    catch (const std::exception &e)
    {
        std::fprintf(stderr, "[oracle] error: %s\n", e.what());
        return 1;
    }
    return 0;
}
