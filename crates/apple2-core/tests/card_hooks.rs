//! Hooks for peripheral cards that touch memory or are driven by a frontend: DMA through
//! `CardCtx`, `access_cycle` and downcasting a slot's card with `Machine::card_mut`.

use apple2_core::card::{Card, CardCtx};
use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;

/// DMAs `0x5A` to `$0300` (value 0) or to `$C000` (anything else) on any register write.
struct DmaCard {
    ok: bool,
    access: u64,
    cycles: u64,
}

impl Card for DmaCard {
    fn name(&self) -> &'static str {
        "Dma"
    }
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn io_write(&mut self, _lo: u8, value: u8, ctx: &mut CardCtx) {
        self.ok = ctx.dma_write(if value == 0 { 0x0300 } else { 0xC000 }, 0x5A);
        (self.access, self.cycles) = (ctx.access_cycle, ctx.cycles);
    }
    fn io_read(&mut self, _lo: u8, ctx: &mut CardCtx) -> u8 {
        ctx.dma_read(0x0300)
    }
}

struct Other;
impl Card for Other {
    fn name(&self) -> &'static str {
        "Other"
    }
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

#[test]
fn cards_can_dma_and_be_downcast() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    assert!(m.insert_card(
        4,
        Box::new(DmaCard {
            ok: false,
            access: 0,
            cycles: 0
        })
    ));
    assert!(m.insert_card(5, Box::new(Other)));
    m.board_mut().write(0xC0C0, 0);
    assert_eq!(m.peek_main(0x300), 0x5A);
    assert_eq!(m.board_mut().read(0xC0C0), 0x5A);
    assert!(m.card_mut::<DmaCard>(4).unwrap().ok);
    m.board_mut().write(0xC0C0, 1); // I/O space is not RAM
    assert!(!m.card::<DmaCard>(4).unwrap().ok);
    assert!(
        m.card_mut::<Other>(4).is_none()
            && m.card_mut::<DmaCard>(3).is_none()
            && m.card_mut::<Other>(5).is_some()
    );
    let c = m.card::<DmaCard>(4).unwrap();
    assert!(c.access <= c.cycles);
}
