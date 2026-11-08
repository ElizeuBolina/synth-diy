#!/usr/bin/env bash
set -euo pipefail

BOOT_MOUNT=/mnt/synth-boot
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT
umask 077

wifi_ssid=$(zenity --entry --title="Synth DIY - Wi-Fi" \
  --text="Nome da rede Wi-Fi 2,4 GHz (SSID):")
wifi_password=$(zenity --password --title="Synth DIY - Senha do Wi-Fi")
pi_user=$(zenity --entry --title="Synth DIY - Usuário" \
  --text="Usuário para acessar a Raspberry Pi:" --entry-text="synth")
pi_password=$(zenity --password --title="Synth DIY - Senha da Raspberry Pi" \
  --text="Defina uma senha forte para o usuário $pi_user:")

if [[ -z "$wifi_ssid" || -z "$wifi_password" || -z "$pi_user" || -z "$pi_password" ]]; then
  zenity --error --title="Synth DIY" --text="Todos os campos são obrigatórios."
  exit 1
fi

if [[ ! "$pi_user" =~ ^[a-z_][a-z0-9_-]*$ ]]; then
  zenity --error --title="Synth DIY" \
    --text="Usuário inválido. Use letras minúsculas, números, _ ou -."
  exit 1
fi

yaml_quote() {
  local value=$1
  value=${value//\\/\\\\}
  value=${value//\"/\\\"}
  printf '"%s"' "$value"
}

password_hash=$(openssl passwd -6 "$pi_password")
ssid_yaml=$(yaml_quote "$wifi_ssid")
wifi_password_yaml=$(yaml_quote "$wifi_password")
user_yaml=$(yaml_quote "$pi_user")
hash_yaml=$(yaml_quote "$password_hash")

cat >"$TMP_DIR/user-data" <<EOF
#cloud-config
hostname: synth
manage_etc_hosts: true
ssh_pwauth: true
timezone: America/Sao_Paulo
locale: pt_BR.UTF-8
users:
  - name: $user_yaml
    gecos: Synth DIY
    groups: [adm, audio, dialout, gpio, i2c, input, netdev, plugdev, spi, sudo, users, video]
    shell: /bin/bash
    lock_passwd: false
    passwd: $hash_yaml
    sudo: ALL=(ALL) NOPASSWD:ALL
packages:
  - avahi-daemon
EOF

cat >"$TMP_DIR/network-config" <<EOF
network:
  version: 2
  ethernets:
    eth0:
      dhcp4: true
      optional: true
  wifis:
    wlan0:
      dhcp4: true
      optional: false
      regulatory-domain: BR
      access-points:
        $ssid_yaml:
          password: $wifi_password_yaml
EOF

pkexec sh -c '
  set -e
  test -d "$1"
  install -m 0644 "$2/user-data" "$1/user-data"
  install -m 0600 "$2/network-config" "$1/network-config"
  touch "$1/ssh"
  sync
' sh "$BOOT_MOUNT" "$TMP_DIR"

zenity --info --title="Synth DIY" \
  --text="Configuração concluída. O cartão pode ser removido com segurança após ser desmontado."
