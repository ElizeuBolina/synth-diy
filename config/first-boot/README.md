# Configuração de primeiro boot

Arquivos gravados na partição `bootfs` do Raspberry Pi OS Lite:

- `user-data`: cria o usuário, define hostname, fuso horário e habilita login SSH por senha.
- `network-config`: conecta a Raspberry Pi ao Wi-Fi no primeiro boot.
- arquivo vazio `ssh`: habilita o serviço SSH.

Esta configuração foi gravada e verificada no microSD em 4 de junho de 2026.

## Credenciais

- Hostname: `synth`
- Endereço esperado: `synth.local`
- IP cabeado validado: `192.168.1.10`
- IP Wi-Fi validado: `192.168.1.11`
- Usuário SSH: `synth`
- Senha SSH inicial: `synth`
- Wi-Fi SSID: `Desktop_F1412964`
- Senha Wi-Fi: `<SENHA_WIFI>`

Para aplicar os arquivos em um novo cartão já gravado:

```bash
sudo cp config/first-boot/user-data /caminho/bootfs/user-data
sudo cp config/first-boot/network-config /caminho/bootfs/network-config
sudo touch /caminho/bootfs/ssh
sync
```

Trocar a senha inicial depois do primeiro login:

```bash
passwd
```

## Primeiro acesso

Após inserir o cartão e ligar a Raspberry Pi, aguardar até 5 minutos:

```bash
ping synth.local
ssh synth@synth.local
```

Se o nome `synth.local` não funcionar, localizar o IP no roteador e executar:

```bash
ssh synth@IP_DA_RASPBERRY
```

## Estado validado

Em 4 de junho de 2026, o SSH foi validado por:

```bash
ssh synth@192.168.1.10
ssh synth@192.168.1.11
```

Depois da preparação inicial:

- Acesso por chave SSH local configurado.
- `sudo` sem senha configurado para o usuário `synth`.
- Kernel atualizado para `6.18.33+rpt-rpi-v7`.
- Rust instalado: `rustc 1.96.0`, `cargo 1.96.0`.
- FluidSynth instalado: `2.4.4`.
- Serviço automático genérico do FluidSynth desabilitado.
- DAC USB ainda pendente de conexão/teste.

A conexão Wi-Fi não foi criada automaticamente pelo cloud-init. Ela foi criada diretamente na Raspberry Pi com:

```bash
sudo nmcli dev wifi connect "Desktop_F1412964" password "<SENHA_WIFI>" ifname wlan0
```
