//! The oracle's `--state` path, end to end (HU-0201, framebuffer differential):
//!
//! ```text
//! cargo run --release -p apple2-video --example aws_state -- save hires-noise /tmp/s.aws.yaml 2
//! tools/conformance/oracle/aws-for-oracle.py /tmp/s.aws.yaml /tmp/s-oracle.aws.yaml
//! audetto-oracle --state /tmp/s-oracle.aws.yaml --frames 2 --out /dev/null --cards none --fb-out /tmp/ora
//! cargo run --release -p apple2-video --example aws_state -- render /tmp/s.aws.yaml 2 4   # writes target/vd/state-rust.rgba
//! cmp /tmp/ora.rgba target/vd/state-rust.rgba
//! ```
//!
//!   save SCENE OUT FRAMES      runs a scene from power-on and saves it (no extension block)
//!   render IN FRAMES TYPE      loads a state, renders FRAMES frames with video type TYPE (0-8),
//!                              writes the framebuffer as RGBA8 to target/vd/state-rust.rgba
use apple2_core::{Machine, MachineConfig, Model};
use apple2_savestate::{load_aws_yaml, save_aws_yaml, ImageRequest, LoadHost, SaveOptions};
use apple2_video::{RenderConfig, Renderer, VideoType};

#[path = "../tools/scenes.rs"]
mod scenes;

struct NoImages;
impl LoadHost for NoImages {
    fn open_image(
        &mut self,
        _r: &ImageRequest,
    ) -> Result<apple2_storage::image::DiskImage, String> {
        Err("none".into())
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a[1] == "save" {
        let def = scenes::all().into_iter().find(|s| s.name == a[2]).unwrap();
        let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
        for (ad, b) in def.main.iter().chain(&[(scenes::ORG, def.code.clone())]) {
            for (i, v) in b.iter().enumerate() {
                m.poke_main(ad + i as u16, *v);
            }
        }
        for (ad, b) in &def.aux {
            for (i, v) in b.iter().enumerate() {
                m.poke_aux(ad + i as u16, *v);
            }
        }
        m.cpu_mut().pc = scenes::ORG;
        let frames: u64 = a[4].parse().unwrap();
        while m.frames() < frames {
            m.step();
        }
        let opts = SaveOptions {
            extension: false,
            ..Default::default()
        };
        let saved = save_aws_yaml(&m, &opts).unwrap();
        std::fs::write(&a[3], saved.text).unwrap();
        println!("saved at cycle {} frame {}", m.cycles(), m.frames());
    } else {
        let text = std::fs::read_to_string(&a[2]).unwrap();
        let loaded = load_aws_yaml(&text, &mut NoImages).unwrap();
        let mut m = loaded.machine;
        let frames: u64 = a[3].parse().unwrap();
        let vt = VideoType::ALL[a[4].parse::<usize>().unwrap()];
        let mut r = Renderer::new(RenderConfig {
            video_type: vt,
            ..Default::default()
        });
        let target = m.frames() + frames;
        println!("loaded at cycle {} frame {}", m.cycles(), m.frames());
        r.resync(m.cycles_in_frame());
        while m.frames() < target {
            r.run_frame(&mut m);
        }
        let mut bytes = Vec::new();
        for &c in r.pixels() {
            bytes.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]);
        }
        std::fs::create_dir_all("target/vd").unwrap();
        std::fs::write("target/vd/state-rust.rgba", &bytes).unwrap();
        println!("cycles {}", m.cycles());
    }
}
