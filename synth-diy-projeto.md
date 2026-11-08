# Sintetizador DIY — Teclado de Órgão + Raspberry Pi 3

Projeto de instrumento standalone: transformar um teclado salvo de um órgão antigo em um sintetizador autônomo, montado em um armário de madeira feito à mão. Uma Raspberry Pi 3 Model B v1.2 fará a leitura do teclado em Rust e usará inicialmente o FluidSynth para produzir áudio a partir de SoundFonts.

---

## Visão geral

- **Objetivo:** instrumento que produz som sozinho (standalone), sem PC, tocando direto num amplificador externo.
- **Hardware base:** teclado de órgão (44 teclas), Raspberry Pi 3 Model B v1.2.
- **Software principal:** controlador em Rust + FluidSynth.
- **Síntese inicial:** reprodução de SoundFonts (`.sf2`).
- **Evolução futura:** motor de síntese próprio em Rust.
- **Gabinete:** armário de madeira de confecção própria.

### Arquitetura do sinal

```
[Matriz 5×12] → GPIO → [Controlador Rust] → [MIDI ALSA] → [FluidSynth] → [Áudio ALSA]
                        (scan+debounce)       (note on/off)     (SoundFont)    (DAC USB → amp → falantes)
```

O controlador Rust e o FluidSynth serão serviços separados. Essa divisão permite testar, reiniciar e substituir cada componente independentemente.

---

## O teclado (matriz mapeada)

Teclado de 44 teclas com dois flats: **5 pinos** (seleção de oitava / barramento) e **12 pinos** (grau cromático C→B).

| Barramento (flat 5) | Oitava | Notas             | Qtd |
|---------------------|--------|-------------------|-----|
| Pino 1              | 0      | F até B (parcial) | 7   |
| Pino 2              | 1      | C até B (completa)| 12  |
| Pino 3              | 2      | C até B (completa)| 12  |
| Pino 4              | 3      | C até B (completa)| 12  |
| Pino 5              | —      | só o C final      | 1   |

**Total: 7 + 12 + 12 + 12 + 1 = 44 teclas.** Começa em F, termina num C (abrangência clássica de manual de órgão).

- O flat de **12 pinos** = grau cromático dentro da oitava (C=0, C#=1, ... B=11).
- O flat de **5 pinos** = seletor de oitava/barramento.
- A placa **já possui diodos**, o que garante n-key rollover (acordes sem ghosting). Não precisa comprar diodos, mas vale ter ~50× 1N4148 de reserva.

### Fórmula de mapeamento

```
nota_midi = NOTA_BASE + (índice_oitava × 12) + grau_cromático
```

Cuidados com os barramentos parciais: barramento 1 usa só graus 5–11 (F em diante); barramento 5 usa só o grau 0 (C). Se o F inicial for F1 (MIDI 29), o C final cai em C5 (MIDI 72) — mas a `NOTA_BASE` é livre.

### Varredura

- **5 GPIOs de saída** (barramentos — energiza um por vez).
- **12 GPIOs de entrada** (lê os graus).
- Total: 17 pinos, dentro da capacidade da Pi 3.
- A direção da varredura (quem driva / quem lê) depende da orientação dos diodos — confirmar com multímetro.
- Colocar um resistor em série de 330 Ω a 1 kΩ em cada uma das 5 linhas acionadas para limitar corrente caso dois GPIOs sejam configurados incorretamente.
- Definir a pinagem BCM antes da montagem, evitando GPIO 0/1 e reservando pinos para botão de shutdown, LED e controles futuros.

### Conceito elétrico de uma tecla

Cada tecla é a interseção de um **BUS** com uma **nota cromática**. Exemplo: a tecla `G` no `BUS 3`.

Tecla solta:

```text
GPIO BUS 3 --[1k]-- o  o -- diodo -- GPIO NOTA G
                    aberto
```

Tecla pressionada:

```text
GPIO BUS 3 --[1k]-- o--o -- diodo -- GPIO NOTA G
                   fechado
```

O software ativa um BUS por vez. Se, durante o `BUS 3`, a entrada `G` mudar de estado, o controlador interpreta:

```text
BUS 3 + G = nota G daquela região do teclado
```

A matriz inteira é a repetição desse circuito para as 44 teclas. Os diodos deixam o sinal passar em um sentido só e evitam notas fantasma ao tocar acordes.

### Ligação proposta na Raspberry Pi

Proposta inicial para a Raspberry Pi 3 Model B v1.2. A pinagem evita GPIO 0/1 e deixa pinos livres para controles. Antes de soldar definitivo, confirmar a orientação dos diodos; isso define se o software usará entradas com `pull-up` ou `pull-down`.

#### Flat de 5 pinos — BUS / regiões

Usar um resistor de **1 kΩ** em série em cada linha BUS.

| Função | GPIO BCM | Pino físico | Ligação |
|--------|----------|-------------|---------|
| BUS 1 | GPIO5  | 29 | GPIO → 1 kΩ → BUS 1 |
| BUS 2 | GPIO6  | 31 | GPIO → 1 kΩ → BUS 2 |
| BUS 3 | GPIO13 | 33 | GPIO → 1 kΩ → BUS 3 |
| BUS 4 | GPIO19 | 35 | GPIO → 1 kΩ → BUS 4 |
| BUS 5 | GPIO26 | 37 | GPIO → 1 kΩ → BUS 5 |

#### Flat de 12 pinos — notas

| Nota | GPIO BCM | Pino físico |
|------|----------|-------------|
| C  | GPIO12 | 32 |
| C# | GPIO16 | 36 |
| D  | GPIO20 | 38 |
| D# | GPIO21 | 40 |
| E  | GPIO17 | 11 |
| F  | GPIO27 | 13 |
| F# | GPIO22 | 15 |
| G  | GPIO23 | 16 |
| G# | GPIO24 | 18 |
| A  | GPIO25 | 22 |
| A# | GPIO18 | 12 |
| B  | GPIO4  | 7  |

Não ligar 5 V, 3,3 V nem GND ao teclado se ele for apenas uma matriz passiva de contatos e diodos. A ligação da matriz usa somente os **17 fios**: 5 BUS + 12 notas.

### Controles físicos reservados

Com 17 GPIOs usados pela matriz, ainda sobram GPIOs suficientes para controles:

- troca de timbre anterior/próximo;
- botão de shutdown seguro;
- LED de status;
- pedal de sustain;
- botão `hold` futuro;
- encoder de volume, se o volume for digital.

Para trocar timbre no FluidSynth, o controlador Rust enviará `Program Change` MIDI:

```text
botão NEXT -> Rust -> MIDI Program Change +1 -> FluidSynth
botão PREV -> Rust -> MIDI Program Change -1 -> FluidSynth
```

Volume físico continua mais simples no caminho analógico, com potenciômetro depois do DAC e antes do amplificador. A Raspberry Pi não possui entrada analógica nativa.

### Entrada P10 para sustain

É possível usar um pedal de sustain comum de teclado através de uma entrada **P10 fêmea mono** no gabinete. Esse P10 não carrega áudio; é apenas contato seco.

Ligação proposta:

```text
P10 TIP    -> GPIO10 / pino físico 19
P10 SLEEVE -> GND    / pino físico 20
```

Configuração no software:

```text
GPIO com pull-up interno
pedal solto        = HIGH
pedal pressionado  = LOW
```

Ao detectar o pedal:

```text
pressionado -> MIDI CC 64 valor 127
solto       -> MIDI CC 64 valor 0
```

Pedais de sustain podem ser normalmente abertos ou normalmente fechados. Se o comportamento ficar invertido, o Rust terá uma opção de polaridade (`normal`/`inverted`) ou fará detecção no boot.

### Sustain x hold

- **Sustain:** igual pedal de piano. Enquanto o pedal está pressionado, notas soltas continuam soando. Ao soltar o pedal, o FluidSynth encerra as notas sustentadas. Implementação via MIDI `CC 64`.
- **Hold:** trava uma nota/acorde até cancelar ou substituir. Será implementado no Rust mantendo uma lista de notas presas e enviando `NoteOff` quando o modo for cancelado.

### Ideias musicais por ser teclado de órgão antigo

O teclado físico é de órgão antigo, com contato por agulha encostando no barramento. Isso combina mais com comportamento de órgão/synth do que com piano dinâmico.

Características e possibilidades:

- **Velocity fixa:** o contato informa basicamente aberto/fechado, então a primeira versão deve usar velocity fixa (`100` ou `127`). Não há velocity real sem dois contatos por tecla ou outro sensor de curso.
- **Debounce musical:** contato mecânico pode repicar. O scanner deve disparar `NoteOn` rápido no primeiro contato válido, ignorar repiques curtos de alguns ms e confirmar soltura antes do `NoteOff`.
- **Key click:** simular o estalo curto de contato de órgão no ataque da nota. Com FluidSynth isso depende do SoundFont; no motor Rust próprio dá para implementar diretamente.
- **Percussão de órgão:** ao tocar uma nota, disparar também um harmônico curto e baixo, como em órgãos Hammond. Exemplo: nota principal + oitava/quinta curta no ataque.
- **Split por BUS:** como a matriz já separa regiões, os BUS podem ter funções musicais diferentes. Exemplo: BUS 1/2 com timbre grave, BUS 3/4 com timbre principal, BUS 5 como C final ou gatilho especial.
- **Drone/hold por região:** uma tecla ou região pode alternar notas contínuas, drones ou acordes presos, combinando com a estética de órgão/synth.

Ordem recomendada:

```text
1. debounce rápido e confiável
2. velocity fixa
3. sustain por P10
4. troca de timbre
5. key click opcional
6. percussão de órgão e split por BUS
```

---

## A Raspberry Pi 3 Model B v1.2 — considerações

- É uma Pi com Linux, CPU quad-core Cortex-A53 de 1,2 GHz e 1 GB de RAM; **não** é um microcontrolador.
- A CPU suporta 64 bits, mas o projeto usará Raspberry Pi OS Lite 32-bit para consumir menos memória e manter compatibilidade simples com o alvo Rust ARMv7.
- Possui Wi-Fi 2,4 GHz e Bluetooth integrados, além de Ethernet 100 Mb/s.
- O objetivo continua sendo um instrumento standalone, não um dispositivo USB MIDI para outro computador.
- **Latência:** varredura via GPIO roda em userspace sobre SO multitarefa → jitter de alguns ms. Mitigável com prioridade RT (SCHED_FIFO) e buffer de áudio pequeno (~128–256 samples).
- **Polifonia:** começar com 16 vozes e ajustar após medir uso de CPU e ocorrências de underrun.
- **GPIO:** os sinais são de 3,3 V. Só conectar diretamente uma matriz de contatos passivos, sem tensão externa.
- **Alimentação:** usar fonte micro-USB 5 V / 2,5 A de boa qualidade.

---

## Stack de software

- **OS:** Raspberry Pi OS Lite **(32-bit / armhf)** — sem GUI, para reduzir consumo de memória e processos concorrentes.
- **Rust** (via rustup).
- **FluidSynth:** motor inicial de reprodução de SoundFonts.
- **ALSA:** áudio e MIDI internos entre o controlador e o FluidSynth.
- **Crates iniciais:**
  - `rppal` — GPIO, varredura da matriz e botões.
  - crate ALSA para Rust — envio de eventos MIDI ao FluidSynth.
- **Crates futuros:** `cpal` e `ringbuf`/`crossbeam` para síntese própria.
- **Dependências apt:** `build-essential pkg-config libasound2-dev fluidsynth fluid-soundfont-gm alsa-utils curl git`.

> Compilar Rust diretamente na Pi 3 é possível, mas ainda é lento. Quando o projeto crescer, a alternativa será cross-compilar no PC para `armv7-unknown-linux-gnueabihf` e copiar o binário via `scp`.

### Modelo inicial de processos e threads

1. **Serviço controlador Rust** — varre a matriz, aplica debounce, mapeia as teclas e envia NoteOn/NoteOff por MIDI ALSA.
2. **Serviço FluidSynth** — recebe MIDI, renderiza o SoundFont e envia áudio ao DAC USB via ALSA.
3. **`systemd`** — inicia os serviços automaticamente e os reinicia em caso de falha.

### Estrutura do projeto

```
keyboard-synth/
├── Cargo.toml
├── src/
│   ├── main.rs          # setup e ciclo principal
│   ├── matrix.rs        # varredura + debounce → eventos NoteOn/NoteOff
│   ├── mapping.rs       # (barramento, grau) → nota MIDI
│   └── midi.rs          # saída MIDI ALSA para o FluidSynth
├── config/
│   └── synth.env        # dispositivo MIDI, canal e parâmetros
└── systemd/
    ├── fluidsynth.service
    └── keyboard-synth.service
```

---

## Motores de som

### Primeira versão: FluidSynth + SoundFont

- Carrega arquivos `.sf2` com timbres de órgão, piano e sintetizador.
- Recebe eventos MIDI do controlador Rust através do ALSA.
- Fornece polifonia, chorus e reverb já implementados.
- Permite validar rapidamente teclado, latência e cadeia de áudio.

### Evolução futura: síntese própria em Rust

- Oscilador por voz (começar com dente-de-serra ou quadrada).
- ADSR por voz (evita clique no note on/off).
- Pool de ~8–16 vozes com voice stealing.
- Mixer + clamp/limiter simples.
- Saída com `cpal`/ALSA e comunicação lock-free entre scanner e áudio.

---

## Efeitos

Um efeito é um bloco no caminho do sinal (`vozes → mixer → efeito → saída`) — mesma lógica de um pedal de guitarra (a ordem importa). Divisão por custo de CPU na Pi 3:

- **Baratos (sample a sample, sem memória):** ganho, distorção/overdrive (`tanh`), tremolo (LFO no volume).
- **Médios (precisam de buffer do passado):** delay/echo (buffer circular), chorus/flanger (delay curto modulado).
- **Caros:** reverb de qualidade (Schroeder/Freeverb), filtros ressonantes variáveis.

> Conceito-chave: efeitos **sem memória** (cada sample independente) vs. **com memória/estado** (dependem do histórico). Cada efeito empilhado soma carga ao callback, que tem prazo apertado.

### Botão de efeito (futuro)

- Mais um GPIO de entrada (pull-up/down) lido na thread de scan → flag `delay_ativo`.
- **Cuidado:** ligar/desligar abruptamente o mix gera clique/estalo (mesmo motivo do ADSR). Solução: rampa suave de alguns ms no mix do delay.
- Decisão de design: ao "bypassar", cortar tudo na hora **ou** deixar as caudas de eco terminarem naturalmente (gosto pessoal).
- Na primeira versão, reverb e chorus serão fornecidos pelo FluidSynth.

---

## Áudio: saída para o amplificador

**Não usar o P2 nativo da Pi 3 como saída final** — ele gera áudio por PWM e pode produzir chiado audível que o amplificador também amplifica.

Cadeia escolhida:

```
Pi 3 → [DAC USB, line-out P2] → cabo P2→(entrada do amp) → amp mono → falantes
```

- **DAC USB** (recomendado): plug-and-play, line-out limpo, não rouba pinos GPIO. Melhor custo-benefício.
- **DAC I2S (HAT, ex. PCM5102 / HiFiBerry):** qualidade um pouco melhor, mas ocupa GPIO e exige overlay no `/boot/config.txt`.
- **Mono x estéreo:** amp é mono, DAC sai estéreo → usar cabo apropriado pra somar/selecionar canal.
- **No móvel:** prever jack de painel (P2/P10 fêmea fixo na madeira) ligado ao DAC, em vez de pluguinho solto.

*(Pendente: confirmar a entrada do amp mono pra fechar DAC + cabo exatos.)*

---

## Lista de compras (eletrônica)

Presume Pi 3 Model B v1.2 e teclado já em mãos.

### Essencial
- microSD 16–32 GB classe 10.
- Fonte 5V / 2.5A micro-USB (boa — fonte fraca causa reboot).
- DAC USB de áudio (**item mais importante p/ qualidade**).
- Fio sólido + solda pros 17 pinos da matriz (evitar dupont, que solta com vibração).
- Conectores/headers compatíveis com os flats de 5 e 12 pinos.
- 5× resistores de 1 kΩ para proteção das linhas de varredura dos BUS.

### Diodos
- ~50× 1N4148 de reserva (placa já tem diodos).

### Saída de áudio
- Já tem amp mono + falantes → falta DAC + cabo P2→entrada do amp.

### Recomendado / acabamento
- Botão liga-desliga de painel + shutdown seguro por software.
- Entrada P10 fêmea mono para pedal de sustain.
- Pedal de sustain comum de teclado.
- Botões momentâneos para troca de timbre anterior/próximo.
- Potenciômetro de volume (analógico após DAC; ou ADC se for por software).
- LED indicador + resistor 220–330Ω.
- Dissipador/cooler pequeno (armário fechado esquenta).
- Perfboard/régua de bornes pra centralizar ligações.
- Jack de painel (P2/P10 fêmea).
- HDMI + teclado USB temporários pra configuração inicial.

### Ferramentas
- Ferro de solda + estanho, multímetro, alicate de corte/decapador, termorretrátil/fita isolante.

---

## Instalação e primeiro boot

### Concluído no microSD

- [x] RetroPie removido.
- [x] Raspberry Pi OS Lite 32-bit gravado.
- [x] Primeiro boot configurado com hostname, usuário, senha, Wi-Fi e fuso horário.
- [x] SSH habilitado.
- [x] Arquivos de configuração preservados em `config/first-boot/`.
- [x] Cartão sincronizado e desmontado com segurança.

### Primeiro boot na Raspberry Pi

1. Com a Raspberry Pi desligada, inserir o microSD.
2. Conectar o DAC USB, se já estiver disponível.
3. Ligar a fonte por último.
4. Aguardar até 5 minutos para expansão do sistema, criação do usuário e conexão ao Wi-Fi.
5. Do computador, testar:
   ```bash
   ping synth.local
   ssh synth@synth.local
   ```
6. Se `synth.local` não responder, localizar o IP da Raspberry Pi no roteador e usar:
   ```bash
   ssh synth@IP_DA_RASPBERRY
   ```

### Preparação do software após o primeiro acesso

1. Confirmar sistema, arquitetura, rede e espaço:
   ```bash
   hostnamectl
   uname -m
   ip -brief address
   df -h /
   ```
2. Atualizar o sistema e instalar dependências:
   ```bash
   sudo apt update
   sudo apt full-upgrade -y
   sudo apt install -y build-essential pkg-config libasound2-dev \
     fluidsynth fluid-soundfont-gm alsa-utils curl git
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
3. Reiniciar após a atualização:
   ```bash
   sudo reboot
   ```
4. Confirmar o DAC com `aplay -l` e reproduzir um arquivo de teste antes de desenvolver o scanner.
5. Testar o FluidSynth manualmente com o SoundFont padrão.
6. Só depois configurar os serviços `systemd`.

---

## Estado da instalação no microSD

Configuração gravada em 4 de junho de 2026:

- **Imagem:** Raspberry Pi OS Lite 32-bit, baseada em Debian 13 (`trixie`).
- **Hostname:** `synth`.
- **Rede inicial:** Wi-Fi 2,4 GHz configurado para o primeiro boot.
- **Wi-Fi:** SSID `Desktop_F1412964`, senha `<SENHA_WIFI>`.
- **País do Wi-Fi:** `BR`.
- **Fuso horário:** `America/Sao_Paulo`.
- **SSH:** habilitado com autenticação por senha.
- **Usuário inicial:** `synth`.
- **Senha inicial:** `synth`.
- **Endereço esperado na rede local:** `synth.local`.
- **IP cabeado validado:** `192.168.1.10`.
- **IP Wi-Fi validado:** `192.168.1.11`.
- **Situação atual:** primeiro boot concluído, SSH validado por cabo e por Wi-Fi, sistema atualizado e dependências instaladas.
- **Acesso SSH por chave:** configurado para a chave local de `elizeu.bolina@gmail.com`.
- **Sudo:** `synth` configurado com `NOPASSWD` para facilitar automação durante o desenvolvimento.

Observação: a configuração de primeiro boot não criou automaticamente a conexão Wi-Fi via cloud-init. A conexão foi criada manualmente na Pi com NetworkManager:

```bash
sudo nmcli dev wifi connect "Desktop_F1412964" password "<SENHA_WIFI>" ifname wlan0
```

### Estado validado após preparação

Validação feita em 4 de junho de 2026:

- **Kernel após reboot:** `6.18.33+rpt-rpi-v7`.
- **Rust:** `rustc 1.96.0`.
- **Cargo:** `cargo 1.96.0`.
- **FluidSynth:** `2.4.4`.
- **SoundFont GM padrão:** `fluid-soundfont-gm` instalado.
- **Dependências de build/áudio:** `build-essential`, `pkg-config`, `libasound2-dev`, `fluidsynth`, `alsa-utils`, `curl`, `git`.
- **Serviço automático genérico do FluidSynth:** desabilitado para evitar conflito com o serviço dedicado do projeto.
- **Espaço em disco:** ~23 GB livres em `/`.
- **Áudio detectado por enquanto:** P2 nativo (`bcm2835 Headphones`) e HDMI (`vc4-hdmi`).
- **DAC USB:** ainda não detectado em `aplay -l`; conectar o adaptador USB antes do próximo teste de áudio.
- **Teste P2 nativo:** `speaker-test` em `plughw:0,0` executado com sucesso.
- **Teste FluidSynth no P2:** MIDI curto reproduzido via `FluidR3_GM.sf2` em `plughw:0,0` e ouvido fisicamente no falante.
- **Configuração do último teste audível:** nota C4, `PCM` em 78%, `synth.gain=0.8`.
- **Prioridade de áudio:** limites real-time configurados em `/etc/security/limits.d/audio-rt.conf`.
- **Serviço FluidSynth dedicado:** `fluidsynth.service` instalado, habilitado e ativo.
- **Porta MIDI ALSA:** `FLUID Synth (...)`, porta `Synth input port`.
- **Controlador Rust mínimo:** `keyboard-synth` criado, compilado na Pi e testado enviando nota e demo curta ao FluidSynth.

### Projeto Rust inicial

Arquivos criados:

- `keyboard-synth/Cargo.toml`
- `keyboard-synth/src/main.rs`
- `systemd/fluidsynth.service`
- `config/synth.env`

Comandos de teste na Pi:

```bash
cd /home/synth/keyboard-synth
./target/release/keyboard-synth note --note 60 --velocity 110 --ms 1200
./target/release/keyboard-synth demo
```

Fluxo validado:

```text
Rust keyboard-synth → ALSA sequencer MIDI → FluidSynth service → P2 nativo → amplificador/falante
```

> **Segurança:** a senha inicial `synth` é fraca e deve ser trocada logo após o primeiro acesso com o comando `passwd`.

Uma cópia reutilizável da configuração, incluindo credenciais, está em `config/first-boot/`.

---

## Roadmap de execução

1. **Instalar o sistema:** concluído; imagem gravada, primeiro boot, rede, SSH, atualização, Rust e dependências validadas.
2. **Validar áudio:** P2 nativo + FluidSynth validados fisicamente; repetir com DAC USB quando o adaptador estiver disponível.
3. **Validar MIDI pelo Rust:** concluído; programa mínimo envia NoteOn/NoteOff ao FluidSynth.
4. **Provar a leitura:** próximo passo; programa Rust que imprime a tecla pressionada e valida fiação + debounce.
5. **Juntar:** tecla → scanner Rust → MIDI ALSA → FluidSynth → som.
6. **Automatizar:** criar serviços `systemd` e inicialização standalone.
7. **Adicionar controles:** troca de timbre, volume, chorus/reverb e shutdown seguro.
8. **Criar síntese própria:** osciladores, ADSR, filtros e efeitos em Rust.
9. **Montagem no armário:** jack de painel, botões, LED e ventilação.

### Próximo passo imediato

Implementar o módulo de mapeamento e o primeiro scanner GPIO da matriz. Antes de soldar/ligar tudo, confirmar com multímetro a orientação dos diodos para decidir quais 5 linhas serão saídas e quais 12 serão entradas.

```bash
ssh synth@192.168.1.11
cd /home/synth/keyboard-synth
./target/release/keyboard-synth demo
```

Quando o DAC USB estiver disponível, repetir os testes de áudio nele.

---

## Decisões e pendências antes da montagem final

### Decisões fechadas

- Raspberry Pi 3 Model B v1.2.
- Raspberry Pi OS Lite 32-bit.
- Controlador de teclado escrito em Rust.
- FluidSynth + SoundFont como primeiro motor de áudio.
- DAC USB como saída final.
- Serviços separados e inicialização automática por `systemd`.
- Matriz será tratada como 5 BUS + 12 notas, usando 17 GPIOs.
- Entrada P10 de sustain será contato seco para GPIO, enviando MIDI `CC 64`.

### Pendências que exigem teste ou medição

- Confirmar a orientação dos diodos e a direção correta da varredura com multímetro.
- Validar a pinagem BCM proposta na montagem real antes de soldar definitivo.
- Confirmar modelo do DAC USB e nome do dispositivo ALSA.
- Confirmar entrada elétrica do amplificador mono e escolher o cabo correto sem unir diretamente as saídas esquerda e direita.
- Escolher o primeiro SoundFont e medir consumo de CPU, latência e polifonia.
- Decidir quais controles físicos entram na primeira versão além de sustain, troca de timbre, LED e shutdown.
