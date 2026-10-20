//! Parallel printer card (ParallelPrinter.cpp:215-300).

use super::{make, LoadCtx, Mapping};
use crate::yaml::{Map, Writer};
use crate::Error;
use apple2_core::Card;
use apple2_hostio::printer::{ParallelPrinterCard, PrinterEncoding, PrinterOptions, PrinterSnapshot};

/// The card's unit version (:229, :258).
const VERSION: u32 = 1;

pub const MAP: Mapping = Mapping { aw_names: &["Generic Printer"], our_name: "Generic Printer", save, load };

/// `SaveSnapshot` (:225). The print file name is the sink's (AppleWin's default `Printer.txt`
/// is written) and "Enable Dump To Real Printer" is a Windows-only setting (false).
fn save(w: &mut Writer, slot: u8, card: &dyn Card) -> bool {
    let Some(c) = card.as_any().and_then(|a| a.downcast_ref::<ParallelPrinterCard>()) else { return false };
    let s = c.snapshot();
    w.slot(slot, "Generic Printer", VERSION, |w| {
        w.uint("Inactivity", s.inactivity.min(u32::MAX as u64));
        w.uint("Printer Idle Limit", s.options.idle_limit as u64);
        w.string("Print Filename", "Printer.txt");
        w.bool("Is File Open", s.is_open);
        w.bool("Dump To Printer", s.options.dump_to_printer);
        w.bool("Convert Encoding", s.options.convert_encoding.is_some());
        w.bool("Filter Unprintable", s.options.filter_unprintable);
        w.bool("Printer Append", s.options.append);
        w.bool("Enable Dump To Real Printer", false);
    });
    true
}

/// `LoadSnapshot` (:241). The sink comes from the host.
fn load(ctx: &mut LoadCtx, slot: u8, version: u32, st: Map) -> Result<Option<Box<dyn Card>>, Error> {
    if version != VERSION {
        return Err(Error::Format(format!("Version {version} is not supported for card 'Generic Printer'.")));
    }
    let inactivity = st.u64("Inactivity")?;
    let idle_limit = st.u32("Printer Idle Limit")?;
    st.string("Print Filename")?;
    let is_open = st.bool("Is File Open")?;
    // A resumed job is re-opened in append mode and the saved flag is only consumed.
    let append = st.bool("Printer Append")?;
    let dump_to_printer = st.bool("Dump To Printer")?;
    let convert = st.bool("Convert Encoding")?;
    let filter_unprintable = st.bool("Filter Unprintable")?;
    st.bool("Enable Dump To Real Printer")?;
    let Some(mut card) = make(ctx, "Generic Printer", slot) else { return Ok(None) };
    let Some(c) = card.as_any_mut().and_then(|a| a.downcast_mut::<ParallelPrinterCard>()) else {
        return Err(Error::Unsupported("host returned a card that is not a Generic Printer".into()));
    };
    c.apply_snapshot(&PrinterSnapshot {
        inactivity,
        options: PrinterOptions {
            idle_limit,
            dump_to_printer,
            convert_encoding: convert.then_some(PrinterEncoding::Pravets82),
            filter_unprintable,
            append,
        },
        is_open,
    });
    Ok(Some(card))
}
