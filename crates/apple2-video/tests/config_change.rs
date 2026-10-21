//! Changing the render options while running (F9, Ctrl+Shift+F9, the video settings page) takes
//! effect on the next frame and leaves no residue of the previous options:
//! GH#350 ("50% Scan lines" only applied after a video mode change) and
//! GH#611 (holding F9 flickered a line of garbage for one frame).

mod common;
use apple2_core::memory::mf;
use apple2_core::Model;
use apple2_video::{RenderConfig, Renderer, VideoType};
use common::*;

fn scene() -> Fake {
    let mut s = Fake::new(Model::Apple2eEnhanced).hires();
    s.memmode |= mf::HIRES;
    for (i, b) in s.main[0x2000..0x4000].iter_mut().enumerate() {
        *b = (i.wrapping_mul(37) ^ (i >> 3)) as u8;
    }
    s
}

fn fresh(cfg: RenderConfig, s: &Fake, frames: u32) -> Vec<u8> {
    let mut r = Renderer::new(cfg);
    render(&mut r, s, frames);
    r.rgba8()
}

#[test]
fn gh350_half_scanlines_toggle_applies_on_the_next_frame() {
    let s = scene();
    for t in VideoType::ALL {
        let on = RenderConfig {
            video_type: t,
            half_scanlines: true,
            ..Default::default()
        };
        let off = RenderConfig {
            half_scanlines: false,
            ..on.clone()
        };
        let mut r = Renderer::new(on.clone());
        render(&mut r, &s, 2);
        r.set_config(off.clone());
        render(&mut r, &s, 2);
        assert_eq!(r.rgba8(), fresh(off.clone(), &s, 2), "{t:?} on -> off");
        r.set_config(on.clone());
        render(&mut r, &s, 2);
        assert_eq!(r.rgba8(), fresh(on, &s, 2), "{t:?} off -> on");
    }
}

#[test]
fn gh611_cycling_the_video_type_every_frame_leaves_no_garbage() {
    let s = scene();
    let mut r = Renderer::new(RenderConfig::default());
    let mut t = r.config().video_type;
    for _ in 0..27 {
        t = t.next();
        let mut c = r.config().clone();
        c.video_type = t;
        r.set_config(c);
        // The first frame after the switch may differ from a fresh renderer only by the line
        // residue of the old type; the second one must be exact.
        render(&mut r, &s, 2);
        let want = fresh(
            RenderConfig {
                video_type: t,
                ..Default::default()
            },
            &s,
            2,
        );
        assert_eq!(r.rgba8(), want, "{t:?}");
    }
}
