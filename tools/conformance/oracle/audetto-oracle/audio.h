#pragma once
// Audio side of the conformance oracle (HU-0501 / HU-0502): event log, PCM capture, a deterministic
// virtual DirectSound and a tiny 6502 test-program loader. main.cpp only calls the entry points below.

#include <cstdint>
#include <memory>
#include <string>

#include "StdAfx.h"
#include "Interface.h"

namespace audio_oracle
{
    // Consumes `--audio-*` options. Returns true (and advances `i` past the value) when argv[i] is ours.
    bool parseArg(int &i, int argc, char **argv);

    // True when any audio option was given.
    bool enabled();

    // Card type the registry gets for `slot`; `dflt` unless `--audio-card` targets that slot.
    SS_CARDTYPE cardTypeFor(UINT slot, SS_CARDTYPE dflt);

    // CPU batch length in cycles: `dflt` unless `--audio-slice` was given.
    uint32_t slice(uint32_t dflt);

    // Frame hook: the virtual DirectSound buffer for the Mockingboard / SSI263 voices (nullptr when audio
    // is not enabled, which keeps the old "no audio at all" behaviour).
    std::shared_ptr<SoundBuffer> createSoundBuffer(uint32_t bufferSize, uint32_t sampleRate, int channels, const char *name);

    // After CommonInitialisation: sockets, hooks, test program, flags that enable the sound code.
    void begin();

    // Flushes and closes every output file.
    void end();
} // namespace audio_oracle
