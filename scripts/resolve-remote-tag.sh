#!/bin/sh
set -eu

awk '
  NR == 1 { first = $1 }
  $2 ~ /\^\{\}$/ { dereferenced = $1 }
  END { print (dereferenced != "" ? dereferenced : first) }
'
