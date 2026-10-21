# Oráculo de conformidade: AppleWin no SHA congelado, fixtures não redistribuídas e ROMs com proveniência

O projeto afirma ter o mesmo comportamento do AppleWin, mas "o AppleWin" muda a cada commit e o hardware real nem sempre concorda com ele. Para que um resultado de teste (PASS ou FAIL) signifique algo e possa ser repetido, três coisas precisam ser fixadas: contra o que comparamos, de onde vêm os arquivos de teste e de onde vêm as ROMs.

## Oráculo e precedência

O Oráculo padrão é o AppleWin no commit `a8f0d1d40f551ad9e010522c29c2891e33c88275` (17/10/2026), que foi a base do porte. Os executáveis de referência são dois:

- o AppleWin oficial v1.32.1.0 (Windows), executado no Wine 10;
- o fork Linux do audetto (`github.com/audetto/AppleWin`, mesmo núcleo de emulação), compilado a partir de um SHA fixo, usado para gerar Traces de referência em massa.

Entre a tag v1.32.1.0 e o commit a8f0d1d há 5 commits. A lista fica em `docs/conformance/baseline.md`, e qualquer diferença atribuível a eles é registrada ali, não escondida.

Quando o AppleWin e o hardware real divergem, **o AppleWin vence**. Desviar dele exige prova de hardware e o registro como Divergência intencional em `docs/conformance/gaps-and-known-limitations.md`. Sem as duas coisas, o comportamento do AppleWin é o correto, mesmo que pareça um defeito dele. Outros Oráculos (especificação de hardware, Fixture validada, resultado conhecido) são aceitos onde o AppleWin não cobre o caso, e o Manifesto de execução diz qual foi usado.

Mudar o SHA do Oráculo é uma nova decisão: exige uma nova versão do baseline e a reexecução dos testes afetados, e os resultados antigos deixam de valer para o SHA novo.

## Fixtures: o AppleWin-Test não é redistribuído

O repositório `github.com/AppleWin/AppleWin-Test` não tem licença, então não temos permissão para copiá-lo para este repositório. Ele é baixado com SHA fixo para uma pasta de cache fora do controle de versão, `target/conformance-cache/`, e nunca é redistribuído nem incluído em pacotes. No repositório ficam apenas:

- o índice das Fixtures (caminho, SHA-256, modelo, finalidade);
- os hashes do corpus;
- os goldens e Traces de referência gerados por nós.

Se não houver rede e o cache estiver vazio, o resultado dos testes que dependem do corpus é **BLOCKED**, com a causa registrada. Esses testes nunca são pulados em silêncio, nem passam por falta de entrada.

## ROMs mantidas no repositório, com proveniência

As ROMs continuam em `roms/` e embutidas no binário (ADR 0002), como o AppleWin faz em `resource/`. Elas são firmware da Apple e de terceiros distribuído pelo próprio AppleWin; não afirmamos ter licença própria sobre elas. Para que isso seja auditável, `docs/conformance/rom-provenance.md` registra, para cada arquivo, a origem no AppleWin a8f0d1d, o SHA-256 e a conferência de que os bytes são idênticos.

Está planejada uma opção para o usuário fornecer ROMs próprias (a ADR 0002 já prevê que a ROM do usuário chega ao núcleo como bytes). Esta ADR não a implementa; a documentação a trata como "planejada" até que exista.

## Consequências

- Todo teste de conformidade registra no Manifesto o SHA do AppleWin, o SHA do AppleII-Rust e o SHA-256 das ROMs e das Fixtures.
- Uma troca de ROM em `roms/` precisa atualizar a tabela de proveniência e ser revisada, porque muda o resultado de testes de vários Modelos.
- A suíte de conformidade depende de rede na primeira execução (para baixar o AppleWin-Test) e de um Wine 10 funcional para gerar Traces com o executável oficial.
- Em alguns casos seremos mais rígidos do que o hardware (por exemplo, repetindo uma peculiaridade do AppleWin que o hardware não tem). É uma escolha deliberada: o objetivo declarado do projeto é a paridade com o AppleWin.
