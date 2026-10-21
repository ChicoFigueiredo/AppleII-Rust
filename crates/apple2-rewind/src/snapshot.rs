//! Splits a [`MachineState`] into a small JSON `meta` part and a big raw `blob` (RAM, aux RAM,
//! Language Card RAM and the cards' own bytes) so that blobs can be diffed by pages.

use apple2_core::{Machine, MachineState};
use serde::{Deserialize, Serialize};

pub const PAGE: usize = 256;

#[derive(Serialize, Deserialize)]
struct Layout {
    main: usize,
    aux: usize,
    lc: usize,
    cards: Vec<Option<usize>>,
}

pub struct Encoded {
    pub meta: Vec<u8>,
    pub blob: Vec<u8>,
}

pub fn encode(m: &Machine) -> Encoded {
    let mut st = m.save_state();
    let layout = Layout {
        main: st.memory.main.len(),
        aux: st.memory.aux.len(),
        lc: st.memory.lc.len(),
        cards: st
            .slots
            .cards
            .iter()
            .map(|c| c.as_ref().map(Vec::len))
            .collect(),
    };
    let mut blob = Vec::with_capacity(layout.main + layout.aux + layout.lc);
    blob.append(&mut st.memory.main);
    blob.append(&mut st.memory.aux);
    blob.append(&mut st.memory.lc);
    for c in st.slots.cards.iter_mut().flatten() {
        blob.append(c);
    }
    let meta = serde_json::to_vec(&(layout, st)).expect("MachineState is serializable");
    Encoded { meta, blob }
}

pub fn decode(meta: &[u8], blob: &[u8]) -> Option<MachineState> {
    let (layout, mut st): (Layout, MachineState) = serde_json::from_slice(meta).ok()?;
    let mut pos = 0usize;
    let mut take = |n: usize| -> Option<Vec<u8>> {
        let s = blob.get(pos..pos + n)?;
        pos += n;
        Some(s.to_vec())
    };
    st.memory.main = take(layout.main)?;
    st.memory.aux = take(layout.aux)?;
    st.memory.lc = take(layout.lc)?;
    for (slot, len) in st.slots.cards.iter_mut().zip(&layout.cards) {
        if let Some(n) = len {
            *slot = Some(take(*n)?);
        }
    }
    Some(st)
}

fn fnv(mut h: u64, data: &[u8]) -> u64 {
    for b in data {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// 64-bit FNV-1a hash of the complete machine state (everything `save_state` covers).
pub fn state_hash(m: &Machine) -> u64 {
    let e = encode(m);
    fnv(fnv(0xcbf2_9ce4_8422_2325, &e.meta), &e.blob)
}

/// Changed pages of `new` relative to `old` (same length required by the caller).
pub fn diff_pages(old: &[u8], new: &[u8]) -> Vec<(u32, Vec<u8>)> {
    debug_assert_eq!(old.len(), new.len());
    old.chunks(PAGE)
        .zip(new.chunks(PAGE))
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, (_, b))| (i as u32, b.to_vec()))
        .collect()
}

pub fn apply_pages(blob: &mut [u8], pages: &[(u32, Vec<u8>)]) {
    for (i, data) in pages {
        let start = *i as usize * PAGE;
        blob[start..start + data.len()].copy_from_slice(data);
    }
}
