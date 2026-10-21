# Assinatura e notarização

## Estado atual: os pacotes NÃO são assinados

| Plataforma | Estado | Evidência |
|---|---|---|
| Windows | `apple2-rust.exe` **sem assinatura Authenticode**. Não há certificado, nem passo de `signtool` no `release.yml`. | `release.yml`, `packaging/package.sh` |
| macOS | O `.app` recebe só uma assinatura **ad hoc** (`codesign -s -`) quando o `codesign` existe no runner. Não há Developer ID e **não há notarização**. | `packaging/package.sh`, `packaging/README.md` |
| Linux | Sem assinatura de pacote. O verificável é o `SHA256SUMS` da release (integridade, não autenticidade) | `release.yml` |

Esta página não afirma que algum pacote seja "assinado", "notarizado" ou "verificado pela Apple/Microsoft". Nenhuma dessas condições foi obtida, nem testada em máquina nativa (a execução em Windows e macOS está BLOCKED).

## O que o usuário vê hoje (esperado; não testado em máquina nativa)

- **Windows**: o SmartScreen tende a mostrar "O Windows protegeu o computador" para um `.exe` baixado e sem reputação; o usuário usa "Mais informações" e "Executar assim mesmo". Antivírus podem sinalizar executáveis novos e sem assinatura.
- **macOS**: o Gatekeeper recusa um app baixado sem notarização; o usuário abre uma vez com clique direito e "Abrir" (ou remove a quarentena com `xattr -d com.apple.quarantine`). No Apple Silicon, o binário precisa ao menos da assinatura ad hoc, que existe.

Essas mensagens são comportamento documentado dos sistemas, **não verificado aqui**.

## Passos e custos para assinar

Valores aproximados, a conferir nos sites oficiais antes de decidir.

### Windows (Authenticode)

1. Obter um certificado de assinatura de código. Opções: certificado OV ou EV de uma CA (centenas de dólares por ano; EV exige token de hardware/HSM e dá reputação imediata no SmartScreen), ou serviço de assinatura na nuvem (por exemplo Azure Trusted Signing / Artifact Signing, assinatura mensal na faixa de dezenas de dólares, sujeita a elegibilidade e verificação de identidade da organização ou do indivíduo).
2. Guardar o certificado ou as credenciais como segredo do repositório.
3. No `release.yml`, após o build Windows e antes do `package.sh`, rodar `signtool sign /fd SHA256 /tr <timestamp-url> /td SHA256 apple2-rust.exe` (ou a ação do serviço de nuvem), e `signtool verify /pa`.
4. Carimbo de tempo (timestamp) obrigatório, para a assinatura sobreviver à expiração do certificado.
5. Mesmo assinado, um certificado OV só acumula reputação no SmartScreen com o tempo.

### macOS (Developer ID e notarização)

1. Inscrição no Apple Developer Program (US$ 99 por ano; exige conta Apple e, para organização, verificação).
2. Criar o certificado "Developer ID Application" e exportá-lo (.p12) como segredo do repositório.
3. No runner macOS: importar o certificado num keychain temporário; `codesign --force --deep --options runtime --timestamp -s "Developer ID Application: ..." AppleII-Rust.app` (Hardened Runtime; podem ser necessários entitlements para wgpu/Metal e áudio, a testar).
4. Notarizar: `xcrun notarytool submit app.zip --apple-id ... --team-id ... --password <senha de app> --wait`, depois `xcrun stapler staple AppleII-Rust.app` e `spctl --assess --type execute -vv`.
5. Empacotar com `ditto` (já feito) depois do staple.
6. Só nesse ponto se poderá dizer que o app está notarizado, e apenas com a saída do `spctl`/`stapler` como evidência.

### Linux

Opcional: assinar o `SHA256SUMS` com GPG ou Sigstore (`cosign sign-blob`, sem custo de certificado, com identidade OIDC do GitHub). Não implementado.

## Decisão pendente (do dono)

Assinar exige gastar dinheiro e guardar segredos no repositório; isso não foi feito nem configurado. Enquanto não houver decisão, o README e as notas de release devem dizer que os pacotes não são assinados.
