# ROMs embutidas no binário

As ROMs de sistema e de cartas (copiadas do AppleWin para `roms/`) são embutidas com `include_bytes!` no crate `apple2-roms`, como o AppleWin faz com os recursos do `.exe`. A alternativa era ler arquivos em runtime. Ela deixaria o binário menor, mas exigiria resolver caminhos e empacotamento diferentes em Windows, Linux e macOS. A ROM customizada do usuário não muda essa regra: ela chega ao núcleo como bytes na configuração da Máquina, e o núcleo nunca abre arquivos.
