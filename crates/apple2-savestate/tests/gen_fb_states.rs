//! Gera estados sintéticos para comparar o barramento flutuante com o oráculo (uso manual).
//! `GEN_FB_DIR=/dir cargo test -p apple2-savestate --test gen_fb_states -- --ignored`
use apple2_core::{Machine, MachineConfig, Model};
use apple2_savestate::{save_aws_yaml, SaveOptions};

#[test]
#[ignore = "gerador manual"]
fn gen() {
    let dir = std::env::var("GEN_FB_DIR").expect("GEN_FB_DIR");
    for (name, cyc, model) in [
        ("fb_e_0", 0u32, Model::Apple2eEnhanced),
        ("fb_e_5000", 5000, Model::Apple2eEnhanced),
        ("fb_e_12300", 12300, Model::Apple2eEnhanced),
        ("fb_p_0", 0, Model::Apple2Plus),
        ("fb_p_9000", 9000, Model::Apple2Plus),
    ] {
        let mut m = Machine::new(MachineConfig::for_model(model));
        for a in 0..0xC000u32 {
            m.poke_main(a as u16, ((a.wrapping_mul(0x9E37)) >> 8) as u8 | 0x01);
        }
        // $0300: 150 x LDA $C054 (page 1 switch, harmless), then JMP $0300
        let mut pc = 0x300u16;
        for _ in 0..150 {
            for b in [0xAD, 0x54, 0xC0] {
                m.poke_main(pc, b);
                pc += 1;
            }
        }
        for b in [0x4C, 0x00, 0x03] {
            m.poke_main(pc, b);
            pc += 1;
        }
        m.cpu_mut().pc = 0x300;
        let mut text = save_aws_yaml(
            &m,
            &SaveOptions {
                extension: false,
                ..Default::default()
            },
        )
        .unwrap()
        .text;
        let line = text
            .lines()
            .find(|l| l.trim_start().starts_with("Cycles This Frame:"))
            .unwrap()
            .to_string();
        text = text.replace(&line, &format!("      Cycles This Frame: {cyc}"));
        std::fs::write(format!("{dir}/{name}.aws.yaml"), text).unwrap();
    }
}
