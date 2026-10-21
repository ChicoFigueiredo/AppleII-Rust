#pragma once
// Audio observation points of the conformance oracle (see audetto-audio.patch and audio.cpp).
//
// This header is copied by build-audetto-oracle.sh to <fork>/source/audio_hook.h and included by the
// few fork sources the patch touches. Every pointer is null unless the oracle's audio module installs
// it, so a stock fork build pays one pointer test per observation point.

#include <cstdint>

enum OracleAudioKind
{
    OAE_IO_W = 1,       // a=slot, b=addr low byte, c=value        write to the card page $Cs00-$CsFF
    OAE_IO_R,           // a=slot, b=addr low byte, c=value        read from the card page (value returned)
    OAE_AY_W,           // a=slot, b=(unit<<4)|ay, c=reg, d=value  AY register write (_AYWriteReg)
    OAE_AY_R,           // a=slot, b=(unit<<4)|ay, c=reg, d=value  AY register read (AYReadReg)
    OAE_IRQ,            // a=source (eIRQSRC), b=1 asserted / 0 deasserted
    OAE_SPK_TOGGLE,     // speaker toggle
    OAE_PHONEME_START,  // a=slot, b=device, c=phoneme as passed to Play(), d=length in samples
    OAE_PHONEME_END,    // a=slot, b=device                         SSI263::UpdateIRQ
    OAE_PHASOR_IO,      // a=slot, b=addr low byte, c=bWrite        Phasor mode select $C0(8+s)x
};

typedef void (*OracleAudioEventFn)(int kind, int a, int b, int c, int d);
typedef void (*OracleMbPcmFn)(const short *stereoFrames, unsigned numFrames);          // 44.1 kHz, L/R interleaved
typedef void (*OracleSpkrPcmFn)(const short *frames, unsigned numFrames, int channels); // 44.1 kHz
typedef void (*OracleSsiPcmFn)(int slot, int device, const short *mono, unsigned numSamples); // 22.05 kHz

inline OracleAudioEventFn g_pfnOracleAudioEvent = nullptr;
inline OracleMbPcmFn g_pfnOracleMbPcm = nullptr;
inline OracleSpkrPcmFn g_pfnOracleSpkrPcm = nullptr;
inline OracleSsiPcmFn g_pfnOracleSsiPcm = nullptr;

#define ORACLE_AUDIO_EVENT(kind, a, b, c, d)                                                                           \
    do                                                                                                                 \
    {                                                                                                                  \
        if (g_pfnOracleAudioEvent)                                                                                     \
            g_pfnOracleAudioEvent((kind), (int)(a), (int)(b), (int)(c), (int)(d));                                      \
    } while (0)
