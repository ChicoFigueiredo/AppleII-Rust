# Importação de save-states reais do AppleWin (HU-0401)

Gerado por `tools/conformance/savestate-report.py` a partir de `savestate-compare.py`. Oráculo: AppleWin (fork audetto no SHA fixo, ver `docs/adr/0004-oraculo-de-conformidade.md`) carregando o mesmo YAML (`run-audetto-oracle.sh --state ... --cycles 100000 --video-update`). Corpus AppleWin-Test: SHA `4b113ce8cff1426bda5595cac9cb04776adfbb5d` (não redistribuído).

Duas verificações por arquivo:

1. **Formato** (`conformance check-import`): logo após importar, registradores, flags, memória principal e AUX byte a byte, bancos LC/Saturn, chaves (leituras de `$C011-$C01F`) e o estado das cartas (reescrita da máquina no formato AppleWin comparada campo a campo com o YAML). Campos que o porte não modela ou normaliza de propósito ficam em `NOT_MODELLED` em `crates/apple2-conformance/src/import_check.rs`.
2. **Trace** após importar: `conformance trace --state` contra o oráculo, 100000 ciclos, comparação linha a linha (ciclo, PC, opcode, A, X, Y, SP, P). PASS só quando o trace inteiro bate; FAIL na primeira divergência; BLOCKED quando falta imagem, há dependência de hardware ou de tempo real, ou o oráculo não consegue carregar.

## Totais (trace)

| resultado | arquivos |
|---|---|
| PASS | 75 |
| FAIL | 13 |
| BLOCKED | 27 |
| total | 115 |

## Por arquivo

| arquivo | trace | bate até (ciclos) | formato | detalhe |
|---|---|---|---|---|
| tests/conformance/inputs/applewin-real/apple2ee-applesoft-prompt.aws.yaml | PASS | 100000 | PASS | 26700 instruções idênticas; rows_sha256=0c2ff04d60f45aee072094f3e24f71f2d2eff7e8748cc712c53279d6a3a31ace |
| tests/conformance/inputs/applewin-real/apple2ee-dos33-master-boot.aws.yaml | PASS | 100000 | PASS | 26701 instruções idênticas; rows_sha256=678513c0879961848536e2e5cc9892b9caee3baf7b3f9619d23835d3fe646b71 |
| tests/conformance/inputs/applewin-real/apple2p-applesoft-prompt.aws.yaml | PASS | 100000 | PASS | 26665 instruções idênticas; rows_sha256=b8331c1a827b7fab7ca085f98171e3c4d2865404756c0889e2f91d5487c3068e |
| Projects/GH997-SHR-AppleII-64K/SHR-AppleII-64K-Skyfox.aws.yaml | PASS | 100000 | FAIL (2) | 25281 instruções idênticas; rows_sha256=7fc4aca1562ee99f5f2b0c0eb2fbb9496d6176523f93d67202329f469c581409 |
| v2/4Play-slot4.aws.yaml | PASS | 100000 | FAIL (1) | 40047 instruções idênticas; rows_sha256=be89ae7ab9d632b74a30245aaaa389504a25a1e8af5d6bb098e4746a9d2188b8 |
| v2/A2+-VideoModeSwitch.aws.yaml | PASS | 100000 | PASS | 30102 instruções idênticas; rows_sha256=c81e8ef96c0354770f6ae47bc00c25377df666275a42ed3d2a0a37cb7fcc4c20 |
| v2/A2e-enhanced-VideoModeSwitch.aws.yaml | PASS | 100000 | PASS | 30102 instruções idênticas; rows_sha256=c81e8ef96c0354770f6ae47bc00c25377df666275a42ed3d2a0a37cb7fcc4c20 |
| v2/A2e-internal-C4nn-with-mousecard-(self test).aws.yaml | PASS | 100000 | PASS | 33523 instruções idênticas; rows_sha256=7f05d082ba2282ea4d199be1813f83674db8ba46c01c1cd61ffd4bc15df98855 |
| v2/AS-S1_Turn the disk over(PAL).aws.yaml | PASS | 100000 | PASS | 36036 instruções idênticas; rows_sha256=8deeaed115981714736c1b06cf5a0ea9d35b3ecb42b3d3b0f13ce2cd3e1c59cf |
| v2/AS-S1_Turn the disk over.aws.yaml | PASS | 100000 | PASS | 37515 instruções idênticas; rows_sha256=172af70610ec551dcf655a441b17266d61dfb74ddca09ca119b308edfd1edd25 |
| v2/AS-S2_End credits(PAL).aws.yaml | PASS | 100000 | PASS | 40580 instruções idênticas; rows_sha256=2b7e6484dd5c209318438c04085b99369d6570eaba02684ef98ccc0ad96385b4 |
| v2/AS-S2_End credits.aws.yaml | PASS | 100000 | PASS | 40402 instruções idênticas; rows_sha256=233641d573671158652f6fe41ead22b1ec446c137c2e9231bb2edcd8faa8a11a |
| v2/AZTEC.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: AZTEC.dsk |
| v2/Base64A.aws.yaml | PASS | 100000 | PASS | 24417 instruções idênticas; rows_sha256=d77de782134cbb0c22025c7a68bdc189fe4c0c35977f336227d59cb5986d215c |
| v2/Buttons_KeyData_AnyKeyDown-Test.aws.yaml | PASS | 100000 | PASS | 29300 instruções idênticas; rows_sha256=96fffda3697a4e41025c46e172c211e1184fffca080b0e68575a6b131f84d7a2 |
| v2/CPM2.2(56k)-1.aws.yaml | PASS | 100000 | PASS | 26666 instruções idênticas; rows_sha256=81c47003bccc29d99d471c2fb0193cc381c8b3e65095a0645831a7bd557b2ca9 |
| v2/CPM2.2(56k)-2-mbasic.aws.yaml | PASS | 100000 | PASS | 26666 instruções idênticas; rows_sha256=bb55f33ceca174509bb43bbc33f5b20974dd24a8cd6afd84df140dea937aacc6 |
| v2/CPM2.2(56k)-3-GR-A2+.aws.yaml | BLOCKED | - | PASS | Z80 SoftCard ativo: o trace cobre só a CPU 6502 e o ciclo inicial do oráculo difere do arquivo |
| v2/DD-v0.6.aws.yaml | PASS | 100000 | PASS | 33226 instruções idênticas; rows_sha256=cba5ad33a3b5e1c8a62a9863a0daeecb1721634eb897f2a21913bddb64d720cd |
| v2/FANTAVISION.aws.yaml | PASS | 100000 | PASS | 27137 instruções idênticas; rows_sha256=ea6210baac463942c36ae6a51f8f1da7980e33b24b8ba0ab894151724cce3cd0 |
| v2/Gemstone Warrior.aws.yaml | PASS | 100000 | PASS | 38679 instruções idênticas; rows_sha256=ce64c6abcfcd92d6b2eedd79ab37de88f0237ea8759ab737df0e108b6ff3fb20 |
| v2/HDD-32MB.aws.yaml | PASS | 100000 | FAIL (6) | 30621 instruções idênticas; rows_sha256=fc1afb3a2d40d49008a53fbff9d670fcb443e2b9aeedb729c2e34053a16c213c |
| v2/NoSlotClockTest-v6.aws.yaml | BLOCKED | - | PASS | dependência de tempo real: o No-Slot Clock devolve a hora do hospedeiro (fixa no Rust, real no oráculo) |
| v2/NoSlotClockTest.aws.yaml | BLOCKED | - | PASS | dependência de tempo real: o No-Slot Clock devolve a hora do hospedeiro (fixa no Rust, real no oráculo) |
| v2/OMT-PAL.aws.yaml | PASS | 100000 | PASS | 32892 instruções idênticas; rows_sha256=46e9750e8f8aa24586014dd62f4ee0889eb0cf19c039276d0505f5dbb357a84b |
| v2/Pravets82.aws.yaml | PASS | 100000 | PASS | 26666 instruções idênticas; rows_sha256=a6d28aee09b479f4dce2305c51f8e1ada2f1b0de62d11ba7553b6930eee59b78 |
| v2/Pravets8A.aws.yaml | PASS | 100000 | PASS | 31833 instruções idênticas; rows_sha256=dfdf62e88ad22e7f401a8ebe0391c191ebb07bd20465ede666b83962f1b0fc95 |
| v2/Pravets8M.aws.yaml | PASS | 100000 | PASS | 26666 instruções idênticas; rows_sha256=b79d7b50e9fc512326376c5a07f6b8321a4ff9f08349e7f895dfacd5408c4faa |
| v2/Prince of Persia.aws.yaml | PASS | 100000 | FAIL (7) | 26444 instruções idênticas; rows_sha256=3f2523699c64b1347724f5558c014e4f15e7e015188018351206d3a5edc05f2a |
| v2/Reset_defaults.aws.yaml | PASS | 100000 | PASS | 38679 instruções idênticas; rows_sha256=27c292736b7517d35a5ad06c5a725e042e5689a88d09cde22caf37aa9dd4501e |
| v2/SAM Slot 5.aws.yaml | PASS | 100000 | PASS | 31245 instruções idênticas; rows_sha256=b1943a75fdca577c50aa2b250e623e32ed1df63c59b383a1f488da646fd96c6a |
| v2/SNES_MAX-slot4.aws.yaml | FAIL | 6 | FAIL (1) | SNES MAX: bit de dados do controle lido no meio da leitura \| linha 2: oráculo 182315542,0C54,2A,A0,01,08,E3,B4 / rust 182315542,0C54,2A,E0,01,08,E3,B4 |
| v2/SSC-COM2-tx-rx-tcp.aws.yaml | BLOCKED | - | PASS | dependência de TCP/serial do hospedeiro; o oráculo executa BRK em $C8EA/$CAE1 (ROM de expansão do SSC não restaurada no fork) |
| v2/SSC-TCP-IN#2.aws.yaml | BLOCKED | - | PASS | dependência de TCP/serial do hospedeiro; o oráculo executa BRK em $C8EA/$CAE1 (ROM de expansão do SSC não restaurada no fork) |
| v2/SSC-TCP-tx-rx-tcp.aws.yaml | BLOCKED | - | PASS | dependência de TCP/serial do hospedeiro; o oráculo executa BRK em $C8EA/$CAE1 (ROM de expansão do SSC não restaurada no fork) |
| v2/Sherwood Forest.aws.yaml | PASS | 100000 | PASS | 38679 instruções idênticas; rows_sha256=ce64c6abcfcd92d6b2eedd79ab37de88f0237ea8759ab737df0e108b6ff3fb20 |
| v2/TK3000e.aws.yaml | PASS | 100000 | PASS | 26699 instruções idênticas; rows_sha256=a4b183b3440f85a2ef9c53a46aaa0726844e50daee72c8fec89385a1afd43018 |
| v2/Tests-Various.aws.yaml | PASS | 100000 | PASS | 38679 instruções idênticas; rows_sha256=ce64c6abcfcd92d6b2eedd79ab37de88f0237ea8759ab737df0e108b6ff3fb20 |
| v2/Ultima-V.aws.yaml | PASS | 100000 | PASS | 38679 instruções idênticas; rows_sha256=ce64c6abcfcd92d6b2eedd79ab37de88f0237ea8759ab737df0e108b6ff3fb20 |
| v2/archon_i.aws.yaml | PASS | 100000 | PASS | 34220 instruções idênticas; rows_sha256=296871d827f596c41117d05b3ac96261a1c639568b24c23ba5a5ee33fd6ee275 |
| v2/arkanoid.aws.yaml | PASS | 100000 | PASS | 34884 instruções idênticas; rows_sha256=21c1f477fb9bc6d2f38303ad809b69c9f1ea00d0c6eac24852775b66c51d1f2f |
| v2/audit-A2+-LC.aws.yaml | PASS | 100000 | PASS | 38748 instruções idênticas; rows_sha256=d0d6a63f875ef5d1e7d4fa0a2b2a78be44c4e72c440ce3d7e3bf0fbfb7f0a46c |
| v2/audit-A2+-Saturn.aws.yaml | PASS | 100000 | PASS | 38748 instruções idênticas; rows_sha256=d0d6a63f875ef5d1e7d4fa0a2b2a78be44c4e72c440ce3d7e3bf0fbfb7f0a46c |
| v2/audit-A2-LC.aws.yaml | PASS | 100000 | PASS | 38748 instruções idênticas; rows_sha256=d0d6a63f875ef5d1e7d4fa0a2b2a78be44c4e72c440ce3d7e3bf0fbfb7f0a46c |
| v2/audit-A2.aws.yaml | PASS | 100000 | PASS | 37347 instruções idênticas; rows_sha256=2e9360f3d400efc84d87406f14d2e1ea1881e406360f489bec985e87a5de4a16 |
| v2/audit-A2e-enhanced-aux-empty.aws.yaml | PASS | 100000 | PASS | 38680 instruções idênticas; rows_sha256=93216b1b1ff1d6f7230e246d74628a1a8c420347d26d47c0fd2d7252716a1781 |
| v2/audit-A2e-enhanced-aux-std80.aws.yaml | PASS | 100000 | PASS | 38679 instruções idênticas; rows_sha256=6da9ab6a1e500d599377445f4889bde9bc7613ce196c90f78cc32a124c0af0a1 |
| v2/audit-A2e-enhanced.aws.yaml | PASS | 100000 | PASS | 38679 instruções idênticas; rows_sha256=ce64c6abcfcd92d6b2eedd79ab37de88f0237ea8759ab737df0e108b6ff3fb20 |
| v2/audit-A2e.aws.yaml | PASS | 100000 | PASS | 38683 instruções idênticas; rows_sha256=e3ed96e4e20bb2b6602c2384fe018ca23730633cc23b870c77ed78cc80d2fa0f |
| v2/contiki-apple2-1-request-ip.aws.yaml | BLOCKED | - | FAIL (1) | oráculo não carregou o estado: [oracle] MessageBox: Uthernet Interface: Uthernet interface isn't valid! |
| v2/dazzledraw1-mouse-rom-page0.aws.yaml | FAIL | 16817 | PASS | IRQ do cartão de mouse (evento de VBL) 7 ciclos adiantado ou atrasado \| linha 5088: oráculo 32727109,6CEC,D0,00,0D,00,FB,73 / rust 32727116,0360,D8,00,0D,00,F8,77 |
| v2/dazzledraw1-mouse-rom-page6.aws.yaml | FAIL | 14069 | PASS | IRQ do cartão de mouse (evento de VBL) 7 ciclos adiantado ou atrasado \| linha 4571: oráculo 32710079,6E3D,60,80,A8,14,F7,B0 / rust 32710086,0360,D8,80,A8,14,F4,B4 |
| v2/skm-apple2ee.aws.yaml | BLOCKED | - | PASS | o fork do oráculo não tem Shift Key Mod (unidade Game I/O v4); state-compat.py a remove, então o botão lido difere |
| v2/skm-apple2p.aws.yaml | BLOCKED | - | PASS | o fork do oráculo não tem Shift Key Mod (unidade Game I/O v4); state-compat.py a remove, então o botão lido difere |
| v2/SmartPort-firmware/SmartPort-firmware.aws.yaml | FAIL | 4373 | PASS | SmartPort: valor de status lido de $C0nX difere \| linha 1330: oráculo 366402089,6193,AA,84,00,13,F0,B0 / rust 366402089,6193,AA,82,00,13,F0,B0 |
| v2/DiskFormat/DOS3.2.1_init_enh_nib.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: test0.nib |
| v2/DiskFormat/DOS3.2.1_init_enh_woz1.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: wozv1.woz |
| v2/DiskFormat/DOS3.2.1_init_enh_woz2.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: wozv2.woz |
| v2/DiskFormat/DOS3.3_init_auth_dsk.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: test0.dsk |
| v2/DiskFormat/DOS3.3_init_auth_nib.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: test0.nib |
| v2/DiskFormat/DOS3.3_init_auth_woz2.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: wozv2.woz |
| v2/DiskFormat/DOS3.3_init_enh_dsk.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: test0.dsk |
| v2/DiskFormat/DOS3.3_init_enh_nib.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: test0.nib |
| v2/DiskFormat/DOS3.3_init_enh_woz1.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: wozv1.woz |
| v2/DiskFormat/DOS3.3_init_enh_woz2.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: wozv2.woz |
| v2/DiskFormat/ProDOS_format_auth_dsk.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: test0.dsk |
| v2/DiskFormat/ProDOS_format_auth_nib.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: test0.nib |
| v2/DiskFormat/ProDOS_format_auth_woz2.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: wozv2.woz |
| v2/DiskFormat/ProDOS_format_enh_dsk.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: test0.dsk |
| v2/DiskFormat/ProDOS_format_enh_nib.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: test0.nib |
| v2/DiskFormat/ProDOS_format_enh_woz1.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: wozv1.woz |
| v2/DiskFormat/ProDOS_format_enh_woz2.aws.yaml | BLOCKED | - | BLOCKED | rust: erro: BLOCKED: imagens ausentes: wozv2.woz |
| v2/Debugger-prefix-breakpoints/Debugger-prefix-breakpoints.aws.yaml | PASS | 100000 | PASS | 26700 instruções idênticas; rows_sha256=f7d550ba77875ff4bb092554cebfe8d973c21fb3948d3575375ea99488531a66 |
| v2/HDD-RW-IO-Boundary/HDD-RW-IO-Boundary.aws.yaml | PASS | 100000 | FAIL (5) | 26908 instruções idênticas; rows_sha256=99499870a5e1b526ef609d5730a499786bbc6ddc5493dca9c483790df3f623d0 |
| v2/VidHD/SHR-AppleII-64K-Skyfox.aws.yaml | PASS | 100000 | FAIL (2) | 25281 instruções idênticas; rows_sha256=7fc4aca1562ee99f5f2b0c0eb2fbb9496d6176523f93d67202329f469c581409 |
| v2/RGB card/AppleColorCard-slideshow-lake.aws.yaml | PASS | 100000 | PASS | 26666 instruções idênticas; rows_sha256=52be34f0be26f968f2d3f375afc49731cc81a1aa7a64f1589aa75f7659137fe5 |
| v2/RGB card/dazzledraw1.aws.yaml | FAIL | 24285 | PASS | IRQ do cartão de mouse (evento de VBL) 7 ciclos adiantado ou atrasado \| linha 7877: oráculo 32710078,6CEF,8D,00,0D,00,FB,73 / rust 32710085,0360,D8,00,0D,00,F8,77 |
| v2/RGB card/dragon_wars.aws.yaml | PASS | 100000 | PASS | 37675 instruções idênticas; rows_sha256=a877e5def0f598a6108e47c50a71a55ecfc0c295396fc9dcf3d8a3d41faa9da6 |
| v2/Video ROM/A2+.aws.yaml | PASS | 100000 | PASS | 26666 instruções idênticas; rows_sha256=ef9d11a9713daf5c9abb5bf31d35ea78e0467b7d46e59c636472c9d1e7560e38 |
| v2/Video ROM/A2.aws.yaml | PASS | 100000 | PASS | 26666 instruções idênticas; rows_sha256=522a20ae04da0e2fecea32a9700bf3577afa799f1839e5c0c62c4b71d02c6b0b |
| v2/Video ROM/A2JPlus.aws.yaml | PASS | 100000 | PASS | 26189 instruções idênticas; rows_sha256=c0b1d9639f79e5c5f30deb29d73bf7eccb425a1a4b957efc2d97e47212bd2ee5 |
| v2/Video ROM/A2e-enhanced-EDM.aws.yaml | PASS | 100000 | PASS | 26703 instruções idênticas; rows_sha256=838d51d53dcbdf7bcf26ccec41ed703c4274bd9513db7fa1b6099d50d4abaf9c |
| v2/Video ROM/A2e-enhanced.aws.yaml | PASS | 100000 | PASS | 26700 instruções idênticas; rows_sha256=2d8fdbfea96ed5c0c8eafc5c39e4ebd412d9f7525106d9ab8c3596a2ef77e5ae |
| v2/Video ROM/A2e.aws.yaml | PASS | 100000 | PASS | 31832 instruções idênticas; rows_sha256=5f487a0478fc8dca5de1e455c40728b1ac5cc993b4bc7cb381d27d37bb869e44 |
| v2/Memory/II+_s0(LC)_s3(Saturn)-test1.aws.yaml | PASS | 100000 | PASS | 26873 instruções idênticas; rows_sha256=2593b4ef3a805b20e60e6b40af563be248ff18612941b838d72be1a2a0f88efd |
| v2/Memory/II+_s0(LC)_s3(Saturn)-test2.aws.yaml | PASS | 100000 | PASS | 26871 instruções idênticas; rows_sha256=4e6b341fe6d753d8c9ea6fe80a1bd3e7c28c41acd75f7c9902bab91bbae9b32c |
| v2/Memory/II+_s0(Saturn)_s3(Saturn)-test1.aws.yaml | PASS | 100000 | PASS | 26894 instruções idênticas; rows_sha256=9b353de448adc4e4b3bff5834370f0aa8548d5c5dea779cea7638cedfef7fe2d |
| v2/Memory/II+_s0(Saturn)_s3(Saturn)-test2.aws.yaml | PASS | 100000 | PASS | 26892 instruções idênticas; rows_sha256=6c7a76b4f5689bbf4348589930c7a8408d855b49a6180f29ad21e607dcacc151 |
| v2/Memory/IIee_s0(LC)_s3(Saturn)-test1.aws.yaml | PASS | 100000 | PASS | 26953 instruções idênticas; rows_sha256=abd94df27ad597a8a33146a764d54bc6fc582281121ae8145c4656419cd7104f |
| v2/Memory/IIee_s0(LC)_s3(Saturn)-test2.aws.yaml | PASS | 100000 | PASS | 26951 instruções idênticas; rows_sha256=f051d253b1ca5e5c925c5da761e5d58191bef93061be3489e40e9d3e93030107 |
| v2/Memory/LC-RamWrite-Test-II+.aws.yaml | PASS | 100000 | PASS | 33332 instruções idênticas; rows_sha256=799cc03f4908317de978bc39cb5266545b7457a35f51a1495d83d8e441670e3f |
| v2/Memory/LC-RamWrite-Test-II+Saturn.aws.yaml | PASS | 100000 | PASS | 33332 instruções idênticas; rows_sha256=799cc03f4908317de978bc39cb5266545b7457a35f51a1495d83d8e441670e3f |
| v2/Memory/LC-RamWrite-Test.aws.yaml | PASS | 100000 | PASS | 33332 instruções idênticas; rows_sha256=aaa78dad27df7aed7a9302b873240a5be8b9aa7b36443b23a437101312ba90a9 |
| v2/Memory/RamWorksIII-256KB.aws.yaml | PASS | 100000 | FAIL (1) | 28285 instruções idênticas; rows_sha256=8a896ce75c046b8cd01019e714190d8a65972726b1e8d50f29ba97e201598e68 |
| v2/Memory/RamWorks_r4.aws.yaml | PASS | 100000 | PASS | 27890 instruções idênticas; rows_sha256=e0f02a5bb2baca14780201b34646d335d277d0ea1b27cd409871006cb85f883a |
| v2/Memory/Saturn128-Bank3-2.aws.yaml | PASS | 100000 | PASS | 27993 instruções idênticas; rows_sha256=1bd771b4011afa5840c5ffc15547cf610a8bef6d0bb07b96a70c5232e8965cad |
| v2/Memory/Saturn128.aws.yaml | FAIL | 406 | PASS | Saturn: contagem de bancos do Saturn (AUD-C-001), fora da fronteira deste agente \| linha 118: oráculo 30645974,2ADB,EE,32,00,03,EC,34 / rust 30645974,2ADB,EE,00,00,03,EC,36 |
| v2/Memory/Saturn64.aws.yaml | FAIL | 406 | FAIL (1) | Saturn: contagem de bancos do Saturn (AUD-C-001), fora da fronteira deste agente \| linha 118: oráculo 22561764,28F7,EE,46,00,03,EE,34 / rust 22561764,28F7,EE,00,00,03,EE,36 |
| v2/Mockingboard/6522-one-shot.aws.yaml | FAIL | 115 | PASS | linha 32: oráculo 122820435,6195,C5,00,06,04,EC,36 / rust 122820435,6195,C5,0C,06,04,EC,34 |
| v2/Mockingboard/Cybernoid(MBv3).aws.yaml | PASS | 100000 | PASS | 29874 instruções idênticas; rows_sha256=8b3cb6e3f4bfd26571f437a9ca73bed76aeb3be0dfee55344cc6188012819e73 |
| v2/Mockingboard/Cybernoid-TimerAtC480.aws.yaml | FAIL | 109 | PASS | linha 27: oráculo 1874074031,C3FA,2C,00,00,01,E7,36 / rust 1874074024,C28E,10,00,00,01,EA,32 |
| v2/Mockingboard/Cybernoid.aws.yaml | FAIL | 17887 | PASS | linha 5397: oráculo 17504000,C3FA,2C,93,09,02,F1,35 / rust 17503993,D63A,90,93,09,02,F4,31 |
| v2/Mockingboard/Cybernoid2(Phasorv3).aws.yaml | PASS | 100000 | PASS | 30038 instruções idênticas; rows_sha256=828de8e253e6bf5289af2e3369325ef2d2b6a424ca1c16732698742700ab2fc2 |
| v2/Mockingboard/Cybernoid2-Phasor.aws.yaml | FAIL | 7190 | PASS | linha 2117: oráculo 158879193,C3FA,2C,00,9D,FF,DE,B4 / rust 158879186,E8FF,90,00,9D,FF,E1,B0 |
| v2/Mockingboard/MB-Poll-IFR-Test.aws.yaml | PASS | 100000 | PASS | 33349 instruções idênticas; rows_sha256=71c312433a713242354e426a49857138be6c8ddcf2f83b1ed1e24b0c4b636212 |
| v2/Mockingboard/MB-Poll-IFR-Test2.aws.yaml | PASS | 100000 | PASS | 30639 instruções idênticas; rows_sha256=72d82ae32141d54dee479dfa0b8480c98be231737aae99c380b959df669a7a99 |
| v2/Mockingboard/SC01-2x_phonemes_playing_continuously.aws.yaml | PASS | 100000 | PASS | 26700 instruções idênticas; rows_sha256=4d3457b5cd0e702b9bd5bd8d5855481dd20114767fdec79a66f14769752d1ccf |
| v2/Mockingboard/SSI263-4x_phonemes_playing_continuously-alt.aws.yaml | PASS | 100000 | PASS | 26700 instruções idênticas; rows_sha256=937775f136c9e02f8a1c53878b79ef06ce79f6f74746bde7958bcfde3e20ea71 |
| v2/Mockingboard/SSI263-4x_phonemes_playing_continuously.aws.yaml | PASS | 100000 | PASS | 26700 instruções idênticas; rows_sha256=adfb57cd6770f9311c3b06d2e200543161a23ae9ccb35abced002331c13ffa71 |
| v2/Mockingboard/SSI263-test.aws.yaml | FAIL | 109 | PASS | linha 30: oráculo 78000058,646F,C5,00,06,04,EE,36 / rust 78000058,646F,C5,0C,06,04,EE,34 |
| v2/Mockingboard/irq-pending.aws.yaml | FAIL | 65519 | PASS | linha 19178: oráculo 158104785,605E,29,40,8A,0D,F4,35 / rust 158104785,605E,29,00,8A,0D,F4,37 |
| v2/Mockingboard/phasor-fugue(v9).aws.yaml | PASS | 100000 | PASS | 31766 instruções idênticas; rows_sha256=ee5a8d138bda4c790baa17c51426daf447232c2b13c6cb9b8f81bc39d26d685b |
| v2/Mockingboard/phasor-fugue.aws.yaml | PASS | 100000 | FAIL (1) | 31766 instruções idênticas; rows_sha256=ee5a8d138bda4c790baa17c51426daf447232c2b13c6cb9b8f81bc39d26d685b |
| v2/Mockingboard/phasor-speech.aws.yaml | PASS | 100000 | FAIL (1) | 28572 instruções idênticas; rows_sha256=1c90d0db6a802b6854abd7b70afe88cb8f935beae04a4430ff79eba46bc5a76c |
| v2/Mockingboard/phasor-tone.aws.yaml | PASS | 100000 | FAIL (1) | 26701 instruções idênticas; rows_sha256=0c69df7ae27c8ad731e452fe2188d9ab017c918caa805be16c8ee7f59608d2bc |

## Causas raiz corrigidas durante a importação

- `P` carregado sem o bit B (`CPU.cpp:963` faz `| AF_RESERVED | AF_BREAK`); PLP/RTI também limpavam B (`cpu_instructions.inl`).
- `Cycles This Frame` acima do quadro travava o contador (AUD-C-060): normalizado no load e `>=` no `tick()`.
- RAM não listada em dumps parciais ficava zerada: `MemReset` coloca o padrão de power-on antes do load.
- Controlador de HDD de estados antigos: Hddrvr.bin (v1-3) e Hddrvr-v2.bin (v4) em vez do firmware SmartPort; `FixupUnitNum`.
- Barramento flutuante lia a RAM principal; o AppleWin lê a imagem de leitura da CPU (aux com RAMRD), AUD-C-050.
- Entrada de IRQ/NMI não gera linha de trace (o oráculo registra depois do fetch do opcode); opcode em `$C100-$CFFF` vem do fetch real.
- Ferramental: o oráculo precisava de `--video-update` (sem ele o scanner de vídeo ficava parado e o barramento flutuante era constante), da cwd com as imagens e de `state-compat.py` (remove a unidade Game I/O v4 que o fork não conhece).

