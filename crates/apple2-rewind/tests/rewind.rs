//! HU-1201: integrity of rewind (1000 steps forth and back), deterministic replay with keys,
//! limits and the debugger commands.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_debugger::Debugger;
use apple2_rewind::{commands, state_hash, RewindConfig, RewindError, Timeline};

fn boot_machine() -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    m.power_cycle();
    m
}

/// `LDA $C000 / BPL -5 / STA $C010 / STA $0501 / INC $0500 / JMP $0300`
fn key_counter_machine() -> Machine {
    let mut m = boot_machine();
    let prog = [
        0xAD, 0x00, 0xC0, 0x10, 0xFB, 0x8D, 0x10, 0xC0, 0x8D, 0x01, 0x05, 0xEE, 0x00, 0x05, 0x4C,
        0x00, 0x03,
    ];
    for (i, b) in prog.iter().enumerate() {
        m.poke(0x0300 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x0300;
    m
}

fn small_cfg() -> RewindConfig {
    RewindConfig {
        frames_per_checkpoint: 1,
        max_instrs_between_checkpoints: 64,
        full_every: 4,
        ..Default::default()
    }
}

fn round_trip(mut m: Machine, keys: &[(usize, u8)]) {
    let mut tl = Timeline::new(&m, small_cfg());
    let mut hashes = vec![state_hash(&m)];
    for i in 1..=1000usize {
        tl.step(&mut m).unwrap();
        for (_, k) in keys.iter().filter(|(at, _)| *at == i) {
            tl.key_press(&mut m, *k).unwrap();
        }
        hashes.push(state_hash(&m));
    }
    assert_eq!(tl.instr(), 1000);
    assert!(tl.checkpoint_count() > 10);
    // back one instruction at a time, all the way: every state is bit-identical
    for k in (0..1000usize).rev() {
        tl.step_back(&mut m, 1).unwrap();
        assert_eq!(tl.instr(), k as u64);
        assert_eq!(
            state_hash(&m),
            hashes[k],
            "state differs at instruction {k}"
        );
    }
    // and forward again with the logged inputs
    tl.replay_to_head(&mut m).unwrap();
    assert_eq!(tl.instr(), 1000);
    assert_eq!(state_hash(&m), hashes[1000]);
}

#[test]
fn thousand_steps_round_trip_on_boot() {
    round_trip(boot_machine(), &[]);
}

#[test]
fn thousand_steps_round_trip_with_input() {
    let m = key_counter_machine();
    round_trip(m, &[(100, b'A'), (400, b'B'), (401, b'C'), (700, b'D')]);
}

#[test]
fn rewind_n_forward_n_returns_to_the_same_hash() {
    let mut m = boot_machine();
    let mut tl = Timeline::new(&m, small_cfg());
    for _ in 0..500 {
        tl.step(&mut m).unwrap();
    }
    let h = state_hash(&m);
    tl.step_back(&mut m, 500).unwrap();
    assert_eq!(tl.instr(), 0);
    assert!(tl.is_behind_head());
    tl.replay_to_head(&mut m).unwrap();
    assert!(!tl.is_behind_head());
    assert_eq!(state_hash(&m), h);
}

#[test]
fn deterministic_replay_with_keys_and_frames() {
    let mut m = key_counter_machine();
    let mut tl = Timeline::new(
        &m,
        RewindConfig {
            frames_per_checkpoint: 2,
            ..Default::default()
        },
    );
    let mut keystrokes = 0;
    for f in 0..12 {
        if f % 3 == 1 {
            tl.key_press(&mut m, b'X').unwrap();
            keystrokes += 1;
        }
        tl.run_frame(&mut m).unwrap();
    }
    assert_eq!(m.peek(0x0500), keystrokes);
    assert_eq!(m.peek(0x0501), b'X' | 0x80);
    let final_hash = state_hash(&m);
    let final_cycles = m.cycles();
    // wipe the future: go to the very start, then replay everything
    tl.rewind_to_instr(&mut m, 0).unwrap();
    assert_eq!(m.peek(0x0500), 0);
    tl.replay_to_head(&mut m).unwrap();
    assert_eq!(m.peek(0x0500), keystrokes);
    assert_eq!(m.cycles(), final_cycles);
    assert_eq!(state_hash(&m), final_hash);
    // the log carries the cycle of each key
    let key_cycles: Vec<u64> = tl
        .log()
        .filter(|e| matches!(e.event, apple2_rewind::Event::Key(_)))
        .map(|e| e.cycle)
        .collect();
    assert_eq!(key_cycles.len(), keystrokes as usize);
    assert!(key_cycles.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn rewind_to_cycle_lands_on_an_instruction_boundary() {
    let mut m = boot_machine();
    let mut tl = Timeline::new(&m, small_cfg());
    let mut seen = Vec::new();
    for _ in 0..300 {
        tl.step(&mut m).unwrap();
        seen.push(m.cycles());
    }
    let want = seen[150] + 1; // inside the next instruction
    tl.rewind_to_cycle(&mut m, want).unwrap();
    assert_eq!(m.cycles(), seen[150]);
    tl.rewind_to_cycle(&mut m, seen[100]).unwrap();
    assert_eq!(m.cycles(), seen[100]);
}

#[test]
fn new_input_after_a_rewind_starts_a_new_future() {
    let mut m = key_counter_machine();
    let mut tl = Timeline::new(&m, small_cfg());
    for _ in 0..200 {
        tl.step(&mut m).unwrap();
    }
    tl.key_press(&mut m, b'A').unwrap();
    for _ in 0..200 {
        tl.step(&mut m).unwrap();
    }
    assert_eq!(m.peek(0x0500), 1);
    tl.step_back(&mut m, 350).unwrap();
    assert_eq!(m.peek(0x0500), 0);
    tl.key_press(&mut m, b'B').unwrap();
    assert_eq!(tl.head_instr(), tl.instr());
    for _ in 0..100 {
        tl.step(&mut m).unwrap();
    }
    assert_eq!(m.peek(0x0501), b'B' | 0x80);
    assert_eq!(m.peek(0x0500), 1);
}

#[test]
fn checkpoint_count_limit_drops_the_oldest_and_stays_correct() {
    let mut m = key_counter_machine();
    let cfg = RewindConfig {
        max_instrs_between_checkpoints: 50,
        full_every: 3,
        max_checkpoints: 6,
        ..Default::default()
    };
    let mut tl = Timeline::new(&m, cfg);
    let mut hashes = Vec::new();
    for i in 0..1000 {
        if i == 700 {
            tl.key_press(&mut m, b'Q').unwrap();
        }
        tl.step(&mut m).unwrap();
        hashes.push(state_hash(&m));
    }
    assert!(tl.checkpoint_count() <= 6);
    let oldest = tl.oldest_instr();
    assert!(oldest > 0);
    assert_eq!(
        tl.rewind_to_instr(&mut m, oldest - 1),
        Err(RewindError::OutOfRange { oldest })
    );
    // everything still reachable is exact (the oldest kept delta was promoted to full)
    for target in [oldest, oldest + 1, 800, 999] {
        tl.rewind_to_instr(&mut m, target).unwrap();
        assert_eq!(state_hash(&m), hashes[target as usize - 1]);
    }
}

#[test]
fn ram_limit_is_respected() {
    let mut m = boot_machine();
    let mut tl = Timeline::new(
        &m,
        RewindConfig {
            frames_per_checkpoint: 1,
            full_every: 2,
            max_ram_bytes: 400_000,
            ..Default::default()
        },
    );
    for _ in 0..40 {
        tl.run_frame(&mut m).unwrap();
    }
    assert!(tl.memory_bytes() <= 400_000, "{} bytes", tl.memory_bytes());
    assert!(tl.checkpoint_count() >= 1);
    assert!(
        tl.oldest_instr() > 0,
        "the oldest checkpoints were discarded"
    );
    // lowering the limit later also takes effect at once
    let n = tl.checkpoint_count();
    tl.set_config(RewindConfig {
        max_checkpoints: 2,
        ..tl.config().clone()
    });
    assert!(tl.checkpoint_count() <= 2.min(n));
}

#[test]
fn incremental_checkpoints_are_much_smaller_than_full_ones() {
    let mut m = boot_machine();
    let mut tl = Timeline::new(
        &m,
        RewindConfig {
            frames_per_checkpoint: 1,
            full_every: 1000,
            ..Default::default()
        },
    );
    for _ in 0..20 {
        tl.run_frame(&mut m).unwrap();
    }
    let per_checkpoint = tl.memory_bytes() / tl.checkpoint_count();
    let full = 64 * 1024;
    assert!(per_checkpoint < full, "{per_checkpoint} per checkpoint");
}

#[test]
fn debugger_commands_rewind_and_stepback() {
    let mut m = boot_machine();
    let mut dbg = Debugger::new();
    dbg.initialize(&mut m);
    dbg.enter(&mut m);
    let mut tl = Timeline::new(&m, small_cfg());
    commands::execute(&mut tl, &mut dbg, &mut m, "T 10");
    assert_eq!(tl.instr(), 0x10);
    let h = state_hash(&m);
    commands::execute(&mut tl, &mut dbg, &mut m, "T 4");
    commands::execute(&mut tl, &mut dbg, &mut m, "STEPBACK");
    assert_eq!(tl.instr(), 0x13);
    commands::execute(&mut tl, &mut dbg, &mut m, "REWIND 3");
    assert_eq!(tl.instr(), 0x10);
    assert_eq!(state_hash(&m), h);
    commands::execute(&mut tl, &mut dbg, &mut m, "REPLAY");
    assert_eq!(tl.instr(), 0x14);
    // a memory edit opens a gap but history before it is intact
    commands::execute(&mut tl, &mut dbg, &mut m, "300:EA");
    assert_eq!(m.peek(0x300), 0xEA);
    commands::execute(&mut tl, &mut dbg, &mut m, "STEPBACK 2");
    assert_eq!(tl.instr(), 0x13);
    assert_ne!(m.peek(0x300), 0xEA);
}
