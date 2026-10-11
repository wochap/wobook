#!/usr/bin/env bash
# fzf front-end for wobook, drop-in for buku-fzf.

pick() {
  local enter_label="${1:-open}" pos_file
  pos_file=$(mktemp)
  # shellcheck disable=SC2064
  trap "rm -f '$pos_file'" RETURN
  echo 1 >"$pos_file"

  wobook list --format tsv |
    fzf --delimiter=$'\t' --with-nth=2.. --preview 'wobook --color=always show {1}' --reverse --preview-window=wrap \
      --header "Enter: $enter_label | Ctrl-E: edit | Ctrl-Y: copy | Ctrl-X: delete" \
      --bind "ctrl-x:execute-silent(wobook rm {1}; echo \$(({n}+1)) >'$pos_file')+reload(wobook list --format tsv)" \
      --bind "ctrl-e:execute(wobook edit {1}; echo \$(({n}+1)) >'$pos_file')+reload(wobook list --format tsv)" \
      --bind "ctrl-y:execute-silent(printf %s {1} | wl-copy --trim-newline)+abort" \
      --bind "load:transform:echo \"pos(\$(cat '$pos_file'))\"" |
    cut -f 1
}

if [[ "${1:-}" == "--select" ]]; then
  selected=$(pick copy)

  if [[ -n "$selected" ]]; then
    echo -n "$selected" | wl-copy --trim-newline
  fi
elif [[ -z "${1:-}" || "${1:-}" == "--open" ]]; then
  selected=$(pick)

  if [[ -n "$selected" ]]; then
    xdg-open "$selected" >/dev/null 2>&1
  fi
elif [[ "${1:-}" == "--add" ]]; then
  wobook edit --new
elif [[ "${1:-}" == "--edit" ]]; then
  while true; do
    selected=$(pick edit)

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
