// Framebuffer dump for the conformance oracle (HU-0201/HU-0202).
//
// Kept apart from main.cpp so the CPU-trace work and this one do not conflict. main.cpp only calls
// the four functions below. Options handled here:
//
//   --fb-out PREFIX    write PREFIX.rgba (full framebuffer, RGBA8, top-down) and print the SHA-256
//                      of the visible area and of the whole buffer; this selects framebuffer mode
//                      (no trace is written; run for --frames N, ending on a frame boundary)
//   --scene FILE       scene v1: RAM segments + entry point (see tools/conformance/oracle/README.md)
//   --video-type N     AppleWin VideoType_e (0 mono custom, 1 idealized, 2 RGB card, 3 monitor NTSC,
//                      4 colour TV (default), 5 B&W TV, 6 amber, 7 green, 8 white)
//   --video-hz 50|60   video refresh rate (PAL / NTSC)
//   --rgb-card N       0 Video-7 SL7 (= Apple), 1 Le Chat Mauve EVE, 2 Le Chat Mauve Feline
//   --vidhd SLOT       a VidHD card in SLOT (1-7): 640x400 framebuffer, Super Hi-Res
#pragma once

#include <cstdint>

class Registry;

namespace fbdump
{
    // Consumes argv[i] (and its value) when it is one of the options above; returns true if so.
    bool parseArg(int argc, char **argv, int &i);

    // True when --fb-out was given.
    bool enabled();

    // Writes the video options into the in-memory registry, before CommonInitialisation.
    void configureRegistry(Registry &registry);

    // Loads the scene, runs `frames` frames with per-opcode video update and dumps the framebuffer.
    // Called after CommonInitialisation. Returns the process exit code.
    int run(uint64_t frames);
} // namespace fbdump
