//! Auditoria wave0 (HU-0502): SSI263 contra as issues #1412 e #1372 do AppleWin.
//!
//! Oráculo: AppleWin a8f0d1d, `SSI263.cpp` (Write: :135-250, LoadSnapshot/RepeatPhoneme).
//! Estado: PASS para os dois (testes comportamentais, sem comparação de PCM com o C++).

use apple2_audio::ssi263::{Ssi263, SsiType};
use apple2_audio::SampleClock;

fn clk() -> SampleClock {
    SampleClock::with_clock(44_100, 1_020_484.45)
}

fn peak(s: &Ssi263, from: u64, to: u64) -> f32 {
    let c = clk();
    (from..to)
        .step_by(5)
        .map(|x| s.sample_at(x, &c).abs())
        .fold(0.0, f32::max)
}

/// Repro da issue #1412: `C440: 1 0 0 8 0` (amplitude 8, CTL=0), depois `C443: 80` (PD=1).
#[test]
fn aud_p_ssi263_power_down_silences_continuous_phoneme_gh1412() {
    let c = clk();
    let mut s = Ssi263::new(SsiType::Ap, false);
    s.write(0, 1, 0, &c);
    s.write(1, 0, 0, &c);
    s.write(2, 0, 0, &c);
    s.write(3, 0x08, 0, &c); // CTL H->L: o fonema 1 toca em repetição
    s.write(4, 0, 0, &c); // freq. de filtro fora do valor 'silêncio' do power-on
    assert!(
        peak(&s, 0, 40_000) > 0.0,
        "pré-condição: o fonema contínuo tem de soar"
    );
    s.write(3, 0x80, 1_000, &c); // PD=1
    assert_eq!(
        peak(&s, 2_000, 200_000),
        0.0,
        "power-down tem de silenciar o fonema contínuo"
    );
}

/// Repro da issue #1372: fonema contínuo, salvar, carregar -> deve voltar a tocar.
#[test]
fn aud_p_ssi263_snapshot_restarts_continuous_phoneme_gh1372() {
    let c = clk();
    let mut s = Ssi263::new(SsiType::Ap, false);
    s.write(1, 0, 0, &c);
    s.write(2, 0, 0, &c);
    s.write(4, 0, 0, &c);
    s.write(0, 1, 0, &c);
    s.write(3, 0x08, 0, &c);
    let snap = s.snapshot();

    let mut t = Ssi263::new(SsiType::Ap, false);
    t.apply_snapshot(&snap);
    assert!(
        t.needs_restart(),
        "fonema ativo no snapshot deve ser retomado"
    );
    let now = 5_000_000u64;
    assert!(t.restart(now, &c).is_some());
    assert!(
        peak(&t, now, now + 40_000) > 0.0,
        "depois do load o chip não pode ficar mudo"
    );
}

/// O snapshot de um chip em power-down não pode ressuscitar o som.
#[test]
fn aud_p_ssi263_snapshot_in_power_down_stays_silent() {
    let c = clk();
    let mut s = Ssi263::new(SsiType::Ap, false);
    s.write(0, 1, 0, &c);
    s.write(3, 0x08, 0, &c);
    s.write(3, 0x80, 10, &c);
    let mut t = Ssi263::new(SsiType::Ap, false);
    t.apply_snapshot(&s.snapshot());
    assert!(!t.needs_restart());
}
