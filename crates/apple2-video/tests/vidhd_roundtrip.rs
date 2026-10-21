//! AUD-P-050: a VidHD survives save/load. The framebuffer drawn from a machine and from the same
//! machine after an `.aws.yaml` round trip must be identical (SHR registers, palettes and the aux
//! memory all travel in the file).

use apple2_cards::{VidHd, VidHdSnapshot};
use apple2_core::{Machine, MachineConfig, Model};
use apple2_savestate::{load_aws_yaml, save_aws_yaml, ImageRequest, LoadHost, SaveOptions};
use apple2_video::{RenderConfig, Renderer, VideoType, WithVidHd};

struct NoImages;
impl LoadHost for NoImages {
    fn open_image(
        &mut self,
        _req: &ImageRequest,
    ) -> Result<apple2_storage::image::DiskImage, String> {
        Err("no disk images in this test".into())
    }
}

fn machine(model: Model, new_video: u8) -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(model));
    assert!(m.insert_card(2, Box::new(VidHd::new())));
    m.power_cycle();
    m.card_mut::<VidHd>(2)
        .unwrap()
        .apply_snapshot(&VidHdSnapshot {
            mem_mode: 0,
            screen_color: 0x0F,
            new_video,
            border_color: 0x03,
            shadow: 0x08,
        });
    // A recognisable SHR picture: pixels from a counter, a palette per line, mixed 320/640 lines.
    for a in 0..0x7D00u32 {
        m.poke_aux(0x2000 + a as u16, (a * 7 + (a >> 8)) as u8);
    }
    for line in 0..200u16 {
        m.poke_aux(0x9D00 + line, ((line * 5) & 0x8F) as u8);
    }
    for pal in 0..16u16 {
        for i in 0..16u16 {
            let c = (pal * 0x111 + i * 0x0421) & 0x0FFF;
            m.poke_aux(0x9E00 + pal * 32 + i * 2, c as u8);
            m.poke_aux(0x9E00 + pal * 32 + i * 2 + 1, (c >> 8) as u8);
        }
    }
    // A text screen as well, for the legacy (centred) case.
    for i in 0..960u16 {
        m.poke_main(
            0x400 + (i % 40) + (i / 40 % 8) * 0x80 + (i / 320) * 0x28,
            0xC1 + (i % 26) as u8,
        );
    }
    m
}

fn frame(m: &Machine, video_type: VideoType) -> (Vec<u32>, (usize, usize)) {
    let hd = m.card::<VidHd>(2).expect("VidHD present");
    let view = WithVidHd {
        source: m,
        vidhd: hd,
    };
    let mut r = Renderer::new(RenderConfig {
        video_type,
        ..Default::default()
    });
    r.resync(m.cycles_in_frame());
    r.redraw(&view);
    r.redraw(&view);
    (r.pixels().to_vec(), r.size())
}

fn round_trip(m: &Machine) -> Machine {
    let saved = save_aws_yaml(m, &SaveOptions::default()).unwrap();
    assert!(saved.warnings.is_empty(), "{:?}", saved.warnings);
    let loaded = load_aws_yaml(&saved.text, &mut NoImages).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    loaded.machine
}

#[test]
fn shr_framebuffer_is_identical_after_save_and_load() {
    // NEWVIDEO $C1: SHR on and linearised, on an Apple //e Enhanced (its aux RAM is the picture).
    // The ][/][+ dump of the card's own aux bank is covered by apple2-savestate's vidhd tests.
    {
        let model = Model::Apple2eEnhanced;
        let m = machine(model, 0xC1);
        let (before, size) = frame(&m, VideoType::ColorTv);
        assert_eq!(
            size,
            (680, 436),
            "{model:?}: a VidHD makes the framebuffer 640x400"
        );
        assert!(
            before.iter().any(|&p| p != 0xFF00_0000),
            "{model:?}: SHR picture drawn"
        );
        let m2 = round_trip(&m);
        let hd = m2.card::<VidHd>(2).expect("the VidHD is back");
        assert!(hd.is_shr(), "{model:?}: NEWVIDEO restored");
        assert_eq!(
            hd.snapshot(),
            m.card::<VidHd>(2).unwrap().snapshot(),
            "{model:?}"
        );
        let (after, size2) = frame(&m2, VideoType::ColorTv);
        assert_eq!(size2, size);
        assert!(
            before == after,
            "{model:?}: SHR framebuffer differs after the round trip"
        );
    }
}

#[test]
fn legacy_mode_framebuffer_with_a_vidhd_is_identical_after_save_and_load() {
    // SHR off: the text screen is centred in the 640x400 framebuffer.
    let m = machine(Model::Apple2eEnhanced, 0x00);
    for vt in [
        VideoType::ColorTv,
        VideoType::MonoWhite,
        VideoType::ColorMonitorNtsc,
    ] {
        let (before, _) = frame(&m, vt);
        let m2 = round_trip(&m);
        assert!(!m2.card::<VidHd>(2).unwrap().is_shr());
        let (after, _) = frame(&m2, vt);
        assert!(
            before == after,
            "{vt:?}: legacy framebuffer differs after the round trip"
        );
    }
}

/// DIV-002/DIV-003 (framebuffer differential): Super Hi-Res is switched on by the first opcode of
/// the first frame. AppleWin does not recompute the scanner address at a mode change, so line 0
/// of that frame is drawn from the pointer the legacy (centred, 40 px right and 8 rows down) mode
/// left, and its last 40 pixels run off the end of the row into the row above on screen
/// (AppleWin's framebuffer is bottom-up). Only one SHR row (2*y) is drawn per IIgs line.
#[test]
fn first_shr_line_starts_at_the_legacy_pointer_and_wraps_upwards() {
    let mut m = machine(Model::Apple2eEnhanced, 0x00); // SHR off until the program turns it on
    for (i, b) in [0xA9, 0xC1, 0x8D, 0x29, 0xC0, 0x4C, 0x05, 0x03]
        .iter()
        .enumerate()
    {
        m.poke_main(0x300 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
    let mut r = Renderer::new(RenderConfig {
        video_type: VideoType::ColorIdealized,
        ..Default::default()
    });
    r.resync(m.cycles_in_frame());
    {
        let hd = m.card::<VidHd>(2).unwrap();
        r.init(&WithVidHd {
            source: &m,
            vidhd: hd,
        });
    }
    while m.frames() < 1 {
        let c = m.step();
        let hd = m.card::<VidHd>(2).unwrap();
        r.update(
            &WithVidHd {
                source: &m,
                vidhd: hd,
            },
            c,
        );
    }
    let w = r.size().0;
    let px = |x: usize, y: usize| r.pixels()[y * w + x];
    let black = 0xFF00_0000;
    // Line 0 started at (60, 26): 40 columns of 16 pixels end at x=700, wrapping 20 pixels up.
    assert_ne!(px(60, 26), black);
    assert_ne!(
        px(679, 26),
        black,
        "the first line runs to the right border"
    );
    assert_ne!(px(0, 25), black, "...and wraps onto the row above");
    assert_ne!(px(19, 25), black);
    assert_eq!(px(20, 25), black);
    // The properly placed lines (1 onwards) start at x=20 on rows 2*y, with black rows between.
    assert_ne!(px(20, 20), black);
    assert_eq!(px(20, 21), black);
}
