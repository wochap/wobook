#!/usr/bin/env bash
# fzf front-end for wobook, drop-in for buku-fzf.

pick() {
  wobook list --format tsv |
    fzf --delimiter=$'\t' --with-nth=2.. --preview 'wobook show {1}' --reverse --preview-window=wrap |
    cut -f 1
}

if [[ "${1:-}" == "--select" ]]; then
  selected=$(pick)

  if [[ -n "$selected" ]]; then
    echo -n "$selected" | wl-copy --trim-newline
  fi
elif [[ "${1:-}" == "--open" ]]; then
  selected=$(pick)

  if [[ -n "$selected" ]]; then
    xdg-open "$selected" >/dev/null 2>&1
  fi
elif [[ "${1:-}" == "--add" ]]; then
  wobook edit --new
elif [[ "${1:-}" == "--edit" ]]; then
  while true; do
    selected=$(pick)

    if [[ -n "$selected" ]]; then
      wobook edit "$selected"
    fi

    read -r -n 1 -p "Do you want to continue editing? (y/n) " yn
    case $yn in
    [yY])
      printf "\n"
      ;;
    *)
      exit 0
      ;;
    esac
  done
else
  echo -e "Available Options : --select --open --add --edit"
fi

exit 0
