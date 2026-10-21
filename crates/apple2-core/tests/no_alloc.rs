//! Regression guard for emulation throughput: the per-instruction path must not allocate.
//! Counting allocations is deterministic, unlike a wall-clock threshold on a loaded CI machine.
//! (The ignored `emulated_speed` benchmark in `boot.rs` prints MHz.)

use apple2_core::{Card, CardCtx, Machine, MachineConfig, Model};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        System.realloc(p, l, n)
    }
}

#[global_allocator]
static A: Counting = Counting;

/// A card that answers every access (no scheduling), to exercise the slot I/O dispatch.
struct Dummy;
impl Card for Dummy {
    fn name(&self) -> &'static str {
        "Dummy"
    }
    fn io_read(&mut self, lo: u8, ctx: &mut CardCtx) -> u8 {
        lo ^ ctx.floating_bus
    }
}

fn allocs_over(m: &mut Machine, steps: usize) -> usize {
    let before = ALLOCS.load(Ordering::Relaxed);
    for _ in 0..steps {
        m.step();
    }
    ALLOCS.load(Ordering::Relaxed) - before
}

/// One test fn: the counter is process-wide, so concurrent tests would pollute each other.
#[test]
fn stepping_does_not_allocate() {
    applesoft_loop();
    slot_io();
}

fn applesoft_loop() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        let mut m = Machine::new(MachineConfig::for_model(model));
        for _ in 0..150 {
            m.run_frame(); // boot to the prompt
        }
        assert!(m.paste("FOR I=1 TO 1E9:NEXT\r"));
        for _ in 0..20 {
            m.run_frame();
        }
        assert_eq!(allocs_over(&mut m, 200_000), 0, "{model:?}: Applesoft loop");
    }
}

fn slot_io() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    assert!(m.insert_card(5, Box::new(Dummy)));
    // $0300: LDA $C0D0 / STA $C0D1 / LDA $C000 / JMP $0300 (card read, card write, keyboard read).
    let prog = [
        0xAD, 0xD0, 0xC0, 0x8D, 0xD1, 0xC0, 0xAD, 0x00, 0xC0, 0x4C, 0x00, 0x03,
    ];
    for (i, b) in prog.iter().enumerate() {
        m.poke(0x300 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
    assert_eq!(allocs_over(&mut m, 100_000), 0);
}
