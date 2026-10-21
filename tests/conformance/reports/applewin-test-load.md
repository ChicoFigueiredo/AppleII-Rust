# Carga do corpus AppleWin-Test (1 quadro)

Gerado por `crates/apple2-savestate/tests/applewin_test_corpus.rs`. SHA do AppleWin-Test: `4b113ce8cff1426bda5595cac9cb04776adfbb5d`.

**Isto NÃO é PASS de fidelidade.** OK significa apenas IMPLEMENTED: o arquivo carrega, a Máquina é montada e roda 1 quadro (17.030 ciclos) sem erro nem panic. PASS exige comparar o estado observável (CPU, memória, vídeo, cartas) com o Oráculo, o AppleWin no SHA congelado, e isso ainda não foi feito. OK_COM_AVISOS carregou, mas perdeu algo (cartão sem equivalente ou imagem de disco ausente do corpus). Cartões que dependem do hospedeiro usam saídas em memória.

## Total por resultado

| resultado | arquivos |
|---|---|
| OK | 94 |
| OK_COM_AVISOS | 18 |
| **total** | 112 |

## Por fixture

| fixture | modelo | resultado | erro |
|---|---|---|---|
| Projects/GH997-SHR-AppleII-64K/SHR-AppleII-64K-Skyfox.aws.yaml | Apple][+ | OK |  |
| v2/4Play-slot4.aws.yaml | Enhanced Apple//e | OK |  |
| v2/A2+-VideoModeSwitch.aws.yaml | Apple][+ | OK |  |
| v2/A2e-enhanced-VideoModeSwitch.aws.yaml | Enhanced Apple//e | OK |  |
| v2/A2e-internal-C4nn-with-mousecard-(self test).aws.yaml | Apple//e | OK |  |
| v2/AS-S1_Turn the disk over(PAL).aws.yaml | Enhanced Apple//e | OK |  |
| v2/AS-S1_Turn the disk over.aws.yaml | Enhanced Apple//e | OK |  |
| v2/AS-S2_End credits(PAL).aws.yaml | Enhanced Apple//e | OK |  |
| v2/AS-S2_End credits.aws.yaml | Enhanced Apple//e | OK |  |
| v2/AZTEC.aws.yaml | Enhanced Apple//e | OK_COM_AVISOS | avisos: Disk II: cannot open 'AZTEC.dsk': imagem 'AZTEC.dsk' não está no corpus |
| v2/Base64A.aws.yaml | Base 64A | OK |  |
| v2/Buttons_KeyData_AnyKeyDown-Test.aws.yaml | Enhanced Apple//e | OK |  |
| v2/CPM2.2(56k)-1.aws.yaml | Enhanced Apple//e | OK |  |
| v2/CPM2.2(56k)-2-mbasic.aws.yaml | Enhanced Apple//e | OK |  |
| v2/CPM2.2(56k)-3-GR-A2+.aws.yaml | Apple][+ | OK |  |
| v2/DD-v0.6.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Debugger-prefix-breakpoints/Debugger-prefix-breakpoints.aws.yaml | Enhanced Apple//e | OK |  |
| v2/DiskFormat/DOS3.2.1_init_enh_nib.aws.yaml | Apple][+ | OK_COM_AVISOS | avisos: Disk II: cannot open 'test0.nib': imagem 'test0.nib' não está no corpus |
| v2/DiskFormat/DOS3.2.1_init_enh_woz1.aws.yaml | Apple][+ | OK_COM_AVISOS | avisos: Disk II: cannot open 'wozv1.woz': imagem 'wozv1.woz' não está no corpus |
| v2/DiskFormat/DOS3.2.1_init_enh_woz2.aws.yaml | Apple][+ | OK_COM_AVISOS | avisos: Disk II: cannot open 'wozv2.woz': imagem 'wozv2.woz' não está no corpus |
| v2/DiskFormat/DOS3.3_init_auth_dsk.aws.yaml | Enhanced Apple//e | OK_COM_AVISOS | avisos: Disk II: cannot open 'test0.dsk': imagem 'test0.dsk' não está no corpus |
| v2/DiskFormat/DOS3.3_init_auth_nib.aws.yaml | Enhanced Apple//e | OK_COM_AVISOS | avisos: Disk II: cannot open 'test0.nib': imagem 'test0.nib' não está no corpus |
| v2/DiskFormat/DOS3.3_init_auth_woz2.aws.yaml | Enhanced Apple//e | OK_COM_AVISOS | avisos: Disk II: cannot open 'wozv2.woz': imagem 'wozv2.woz' não está no corpus |
| v2/DiskFormat/DOS3.3_init_enh_dsk.aws.yaml | Enhanced Apple//e | OK_COM_AVISOS | avisos: Disk II: cannot open 'test0.dsk': imagem 'test0.dsk' não está no corpus |
| v2/DiskFormat/DOS3.3_init_enh_nib.aws.yaml | Enhanced Apple//e | OK_COM_AVISOS | avisos: Disk II: cannot open 'test0.nib': imagem 'test0.nib' não está no corpus |
| v2/DiskFormat/DOS3.3_init_enh_woz1.aws.yaml | Enhanced Apple//e | OK_COM_AVISOS | avisos: Disk II: cannot open 'wozv1.woz': imagem 'wozv1.woz' não está no corpus |
| v2/DiskFormat/DOS3.3_init_enh_woz2.aws.yaml | Enhanced Apple//e | OK_COM_AVISOS | avisos: Disk II: cannot open 'wozv2.woz': imagem 'wozv2.woz' não está no corpus |
| v2/DiskFormat/ProDOS_format_auth_dsk.aws.yaml | Enhanced Apple//e | OK_COM_AVISOS | avisos: Disk II: cannot open 'test0.dsk': imagem 'test0.dsk' não está no corpus |
| v2/DiskFormat/ProDOS_format_auth_nib.aws.yaml | Enhanced Apple//e | OK_COM_AVISOS | avisos: Disk II: cannot open 'test0.nib': imagem 'test0.nib' não está no corpus |
| v2/DiskFormat/ProDOS_format_auth_woz2.aws.yaml | Enhanced Apple//e | OK_COM_AVISOS | avisos: Disk II: cannot open 'wozv2.woz': imagem 'wozv2.woz' não está no corpus |
| v2/DiskFormat/ProDOS_format_enh_dsk.aws.yaml | Enhanced Apple//e | OK_COM_AVISOS | avisos: Disk II: cannot open 'test0.dsk': imagem 'test0.dsk' não está no corpus |
| v2/DiskFormat/ProDOS_format_enh_nib.aws.yaml | Enhanced Apple//e | OK_COM_AVISOS | avisos: Disk II: cannot open 'test0.nib': imagem 'test0.nib' não está no corpus |
| v2/DiskFormat/ProDOS_format_enh_woz1.aws.yaml | Enhanced Apple//e | OK_COM_AVISOS | avisos: Disk II: cannot open 'wozv1.woz': imagem 'wozv1.woz' não está no corpus |
| v2/DiskFormat/ProDOS_format_enh_woz2.aws.yaml | Enhanced Apple//e | OK_COM_AVISOS | avisos: Disk II: cannot open 'wozv2.woz': imagem 'wozv2.woz' não está no corpus |
| v2/FANTAVISION.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Gemstone Warrior.aws.yaml | Enhanced Apple//e | OK |  |
| v2/HDD-32MB.aws.yaml | Enhanced Apple//e | OK |  |
| v2/HDD-RW-IO-Boundary/HDD-RW-IO-Boundary.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Memory/II+_s0(LC)_s3(Saturn)-test1.aws.yaml | Apple][+ | OK |  |
| v2/Memory/II+_s0(LC)_s3(Saturn)-test2.aws.yaml | Apple][+ | OK |  |
| v2/Memory/II+_s0(Saturn)_s3(Saturn)-test1.aws.yaml | Apple][+ | OK |  |
| v2/Memory/II+_s0(Saturn)_s3(Saturn)-test2.aws.yaml | Apple][+ | OK |  |
| v2/Memory/IIee_s0(LC)_s3(Saturn)-test1.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Memory/IIee_s0(LC)_s3(Saturn)-test2.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Memory/LC-RamWrite-Test-II+.aws.yaml | Apple][+ | OK |  |
| v2/Memory/LC-RamWrite-Test-II+Saturn.aws.yaml | Apple][+ | OK |  |
| v2/Memory/LC-RamWrite-Test.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Memory/RamWorksIII-256KB.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Memory/RamWorks_r4.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Memory/Saturn128-Bank3-2.aws.yaml | Apple][+ | OK |  |
| v2/Memory/Saturn128.aws.yaml | Apple][+ | OK |  |
| v2/Memory/Saturn64.aws.yaml | Apple][+ | OK |  |
| v2/Mockingboard/6522-one-shot.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Mockingboard/Cybernoid(MBv3).aws.yaml | Enhanced Apple//e | OK |  |
| v2/Mockingboard/Cybernoid-TimerAtC480.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Mockingboard/Cybernoid.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Mockingboard/Cybernoid2(Phasorv3).aws.yaml | Enhanced Apple//e | OK |  |
| v2/Mockingboard/Cybernoid2-Phasor.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Mockingboard/MB-Poll-IFR-Test.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Mockingboard/MB-Poll-IFR-Test2.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Mockingboard/SC01-2x_phonemes_playing_continuously.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Mockingboard/SSI263-4x_phonemes_playing_continuously-alt.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Mockingboard/SSI263-4x_phonemes_playing_continuously.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Mockingboard/SSI263-test.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Mockingboard/irq-pending.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Mockingboard/phasor-fugue(v9).aws.yaml | Enhanced Apple//e | OK |  |
| v2/Mockingboard/phasor-fugue.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Mockingboard/phasor-speech.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Mockingboard/phasor-tone.aws.yaml | Enhanced Apple//e | OK |  |
| v2/NoSlotClockTest-v6.aws.yaml | Enhanced Apple//e | OK |  |
| v2/NoSlotClockTest.aws.yaml | Enhanced Apple//e | OK |  |
| v2/OMT-PAL.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Pravets82.aws.yaml | Pravets82 | OK |  |
| v2/Pravets8A.aws.yaml | Pravets8A | OK |  |
| v2/Pravets8M.aws.yaml | Pravets8M | OK |  |
| v2/Prince of Persia.aws.yaml | Enhanced Apple//e | OK |  |
| v2/RGB card/AppleColorCard-slideshow-lake.aws.yaml | Enhanced Apple//e | OK |  |
| v2/RGB card/dazzledraw1.aws.yaml | Enhanced Apple//e | OK |  |
| v2/RGB card/dragon_wars.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Reset_defaults.aws.yaml | Enhanced Apple//e | OK |  |
| v2/SAM Slot 5.aws.yaml | Enhanced Apple//e | OK |  |
| v2/SNES_MAX-slot4.aws.yaml | Enhanced Apple//e | OK |  |
| v2/SSC-COM2-tx-rx-tcp.aws.yaml | Enhanced Apple//e | OK |  |
| v2/SSC-TCP-IN#2.aws.yaml | Enhanced Apple//e | OK |  |
| v2/SSC-TCP-tx-rx-tcp.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Sherwood Forest.aws.yaml | Enhanced Apple//e | OK |  |
| v2/SmartPort-firmware/SmartPort-firmware.aws.yaml | Enhanced Apple//e | OK |  |
| v2/TK3000e.aws.yaml | TK3000//e | OK |  |
| v2/Tests-Various.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Ultima-V.aws.yaml | Enhanced Apple//e | OK |  |
| v2/VidHD/SHR-AppleII-64K-Skyfox.aws.yaml | Apple][+ | OK |  |
| v2/Video ROM/A2+.aws.yaml | Apple][+ | OK |  |
| v2/Video ROM/A2.aws.yaml | Apple][ | OK |  |
| v2/Video ROM/A2JPlus.aws.yaml | Apple][ J-Plus | OK |  |
| v2/Video ROM/A2e-enhanced-EDM.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Video ROM/A2e-enhanced.aws.yaml | Enhanced Apple//e | OK |  |
| v2/Video ROM/A2e.aws.yaml | Apple//e | OK |  |
| v2/archon_i.aws.yaml | Enhanced Apple//e | OK |  |
| v2/arkanoid.aws.yaml | Enhanced Apple//e | OK |  |
| v2/audit-A2+-LC.aws.yaml | Apple][+ | OK |  |
| v2/audit-A2+-Saturn.aws.yaml | Apple][+ | OK |  |
| v2/audit-A2-LC.aws.yaml | Apple][ | OK |  |
| v2/audit-A2.aws.yaml | Apple][ | OK |  |
| v2/audit-A2e-enhanced-aux-empty.aws.yaml | Enhanced Apple//e | OK |  |
| v2/audit-A2e-enhanced-aux-std80.aws.yaml | Enhanced Apple//e | OK |  |
| v2/audit-A2e-enhanced.aws.yaml | Enhanced Apple//e | OK |  |
| v2/audit-A2e.aws.yaml | Apple//e | OK |  |
| v2/contiki-apple2-1-request-ip.aws.yaml | Enhanced Apple//e | OK |  |
| v2/dazzledraw1-mouse-rom-page0.aws.yaml | Enhanced Apple//e | OK |  |
| v2/dazzledraw1-mouse-rom-page6.aws.yaml | Enhanced Apple//e | OK |  |
| v2/skm-apple2ee.aws.yaml | Enhanced Apple//e | OK |  |
| v2/skm-apple2p.aws.yaml | Apple][+ | OK |  |
