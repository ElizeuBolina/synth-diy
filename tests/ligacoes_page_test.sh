#!/usr/bin/env bash
set -euo pipefail

page="ligacoes-raspberry.html"

test -f "$page"

required_text=(
  "Ligacoes na Raspberry Pi"
  "Flat de 5 pinos"
  "Flat de 12 pinos"
  "BUS 1"
  "GPIO5"
  "C#"
  "GPIO16"
  "Entrada P10 para sustain"
  "Nao ligar 5 V, 3,3 V nem GND ao teclado"
)

for text in "${required_text[@]}"; do
  grep -Fq "$text" "$page"
done
